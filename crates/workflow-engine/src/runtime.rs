//! Núcleo do runtime E2E do engine (T-900-E2E-B2). Feature `dds`: tipos de
//! tópico gerados (DdsString/DdsBoundedSequence); testes in-memory rodam
//! com `--features dds` sobre os backends in-memory de dds-dataspace.
//! Fluxo: intent → decisão → validate → mapper → journal → planner →
//! DAG validate → Plan/State → scheduler materializa laya1- tasks →

#![cfg(feature = "dds")]

use crate::dag::{self, PlanCandidate};
use crate::decision::{validate, DecisionPolicy, Verdict};
use crate::engine::{EngineState, PlanGraph, Planner};
use crate::journal::{AdmitOutcome, Journal};
use crate::qos_mapper::{self, QosMapperPolicy, QosTelemetryRecord};
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_contract::workflow::dds_llm_workflow_v1 as w;
use dds_dataspace::api::{DataSpaceApi, DataSpaceError};
use dds_dataspace::workflow_space::{WorkflowSpaceApi, WorkflowSpaceError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// TaskStatus do V4.
const TS_DONE: i32 = 3;
const TS_FAILED: i32 = 4;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("engine: {0}")]
    Engine(#[from] crate::engine::EngineError),
    #[error("dataspace: {0}")]
    DataSpace(#[from] DataSpaceError),
    #[error("workflow space: {0}")]
    WorkflowSpace(#[from] WorkflowSpaceError),
    #[error("journal: {0}")]
    Journal(#[from] crate::journal::JournalError),
    #[error("planner: {0}")]
    Planner(String),
    #[error("dag: {0}")]
    Dag(#[from] dag::DagError),
}

fn to_s(v: String) -> cyclonedds::DdsString {
    v.try_into()
        .unwrap_or_else(|_| cyclonedds::DdsString::default())
}

fn seq_n<T, const N: usize>(items: Vec<T>) -> cyclonedds::DdsBoundedSequence<T, N> {
    // bounds do contrato já foram validados pelo dag::validate_candidate
    cyclonedds::DdsBoundedSequence::from_vec(items)
        .unwrap_or_else(|_| cyclonedds::DdsBoundedSequence::from_vec(vec![]).expect("vazia cabe"))
}

/// Configuração do runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub decision_policy: DecisionPolicy,
    pub mapper_policy: QosMapperPolicy,
    /// Executar retry em falha (objetivo FAILOVER mapeado).
    pub failover_enabled: bool,
    /// Custo estimado por tentativa (microunidades) p/ reserva do journal.
    pub per_attempt_microunits: u64,
    /// Prazo padrão da task publicada quando o nó não define (ns).
    pub default_task_deadline_ns: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            decision_policy: DecisionPolicy::default(),
            mapper_policy: QosMapperPolicy::default(),
            failover_enabled: true,
            per_attempt_microunits: 1,
            default_task_deadline_ns: 600_000_000_000, // 10 min
        }
    }
}

/// Estado por workflow no runtime.
struct WfRuntime {
    intent: w::WorkflowIntent,
    graph: PlanGraph,
    state: EngineState,
    /// node_id -> prompt_template do candidato (para publicar a Task).
    prompts: HashMap<String, String>,
    failover: bool,
    terminal: bool,
    failed_reason: Option<String>,
    qos: Option<QosTelemetryRecord>,
    /// Revisão de state crescente (cada projeção publica uma revisão nova —
    /// mesma revisão com conteúdo diferente é conflito no WorkflowSpace).
    state_rev: u32,
}

/// O runtime do engine (uma instância por processo — engine único, ADR-004).
pub struct EngineRuntime<DS: DataSpaceApi, WS: WorkflowSpaceApi> {
    dataspace: Arc<DS>,
    wspace: Arc<WS>,
    journal: Journal,
    planner: Box<dyn Planner>,
    cfg: RuntimeConfig,
    workflows: HashMap<String, WfRuntime>,
    /// task_id -> (workflow, node)
    task_index: HashMap<String, (String, String)>,
}

impl<DS: DataSpaceApi, WS: WorkflowSpaceApi> EngineRuntime<DS, WS> {
    pub fn new(
        dataspace: Arc<DS>,
        wspace: Arc<WS>,
        journal: Journal,
        planner: Box<dyn Planner>,
        cfg: RuntimeConfig,
    ) -> Self {
        Self {
            dataspace,
            wspace,
            journal,
            planner,
            cfg,
            workflows: HashMap::new(),
            task_index: HashMap::new(),
        }
    }

    /// Registra UMA intenção (stream): cria o registro aguardando decisão.
    pub async fn on_intent(&mut self, intent: &w::WorkflowIntent) -> Result<(), RuntimeError> {
        let wf = intent.workflow_id.to_string();
        if self.workflows.contains_key(&wf) {
            return Ok(());
        }
        self.workflows.insert(
            wf.clone(),
            WfRuntime {
                intent: intent.clone(),
                graph: PlanGraph {
                    workflow_id: wf,
                    plan_revision: 0,
                    nodes: vec![],
                },
                state: EngineState::default(),
                prompts: HashMap::new(),
                failover: self.cfg.failover_enabled,
                terminal: false,
                failed_reason: None,
                qos: None,
                state_rev: 0,
            },
        );
        Ok(())
    }

    /// Intake de UMA decisão. Retorna true se o workflow foi admitido/aceito.
    pub async fn on_decision(
        &mut self,
        decision: &w::RoutingDecision,
    ) -> Result<bool, RuntimeError> {
        let wf = match self.pending_intent_for(&decision.request_id.to_string()) {
            Some(w) => w,
            None => return Ok(false), // decisão sem intent registrada
        };
        let intent = self.workflows[&wf].intent.clone();
        let verdict = validate(decision, &intent, &self.cfg.decision_policy, None);

        let mapping = match &verdict {
            Verdict::Accept { .. } => qos_mapper::map(&verdict, &intent, &self.cfg.mapper_policy),
            _ => qos_mapper::map_fallback(&intent, &self.cfg.mapper_policy),
        };
        let (qos_record, failover) = match &mapping {
            qos_mapper::QosMapping::Applied(e) => {
                let failover = e.failover_retries && self.cfg.failover_enabled;
                (
                    QosTelemetryRecord::applied(&intent.workflow_id.to_string(), e),
                    failover,
                )
            }
            qos_mapper::QosMapping::Rejected(r) => {
                // Política (fallback) recusou: workflow FAILED com causa.
                let wfr = self.workflows.get_mut(&wf).expect("wf");
                wfr.terminal = true;
                wfr.failed_reason = Some(format!("qos: {r}"));
                self.publish_state(&wf, 5).await?;
                return Ok(false);
            }
        };

        match self.journal.admit_workflow(
            &intent.workflow_id.to_string(),
            &intent.request_id.to_string(),
            &decision.decision_id.to_string(),
            &decision.payload_hash.to_string(),
            &verdict_name(&verdict),
            intent.max_cost_microunits,
            u64::from(intent.max_tokens),
        )? {
            AdmitOutcome::Conflict => {
                tracing::warn!(wf = %intent.workflow_id.to_string(), "decisão conflitante ignorada");
                return Ok(false);
            }
            AdmitOutcome::Replay => return Ok(true),
            AdmitOutcome::Admitted => {}
        }

        let request = crate::engine::PlanRequest {
            workflow_id: intent.workflow_id.to_string(),
            prompt: intent.prompt.to_string(),
        };
        let candidate = self.planner.plan(&request).map_err(RuntimeError::Planner)?;
        dag::validate_candidate(&candidate, &intent, now_ns())?;
        let graph = PlanGraph::from_candidate(&intent.workflow_id.to_string(), 1, &candidate);
        let prompts: HashMap<String, String> = candidate
            .nodes
            .iter()
            .map(|n| (n.node_id.clone(), n.prompt_template.clone()))
            .collect();
        let state = EngineState::recover(&self.journal, &graph)?;

        let wfr = self.workflows.get_mut(&wf).expect("wf");
        wfr.graph = graph;
        wfr.state = state;
        wfr.prompts = prompts;
        wfr.failover = failover;
        wfr.qos = Some(qos_record);

        self.publish_plan(&wf, &candidate).await?;
        self.publish_state(&wf, 3).await?;
        Ok(true)
    }

    /// Tick do scheduler: materializa nós prontos (journal+outbox → Task).
    pub async fn tick_scheduler(&mut self) -> Result<usize, RuntimeError> {
        let mut published = 0;
        let ids: Vec<String> = self.workflows.keys().cloned().collect();
        for wf in ids {
            let ready: Vec<String> = {
                let wfr = &self.workflows[&wf];
                if wfr.terminal {
                    continue;
                }
                let done = wfr.state.done_set();
                wfr.graph
                    .ready_nodes(&done)
                    .iter()
                    .map(|n| n.node_id.clone())
                    .collect()
            };
            for node_id in ready {
                let task_id = {
                    let wfr = self.workflows.get_mut(&wf).expect("wf");
                    match wfr.state.materialize(
                        &self.journal,
                        &wfr.graph,
                        &node_id,
                        self.cfg.per_attempt_microunits,
                    ) {
                        Ok(tid) => tid,
                        Err(crate::engine::EngineError::AlreadyDone(_)) => continue,
                        Err(e) => {
                            return Err(RuntimeError::Journal(
                                crate::journal::JournalError::Corrupted(format!(
                                    "materialize: {e}"
                                )),
                            ))
                        }
                    }
                };
                if self.task_index.contains_key(&task_id) {
                    continue; // já publicada nesta tentativa: aguardar terminal
                }
                self.task_index
                    .insert(task_id.clone(), (wf.clone(), node_id.clone()));
                let (prompt, priority, deadline) = {
                    let wfr = &self.workflows[&wf];
                    let spec = wfr.graph.node(&node_id).expect("nó validado");
                    let prompt = wfr
                        .prompts
                        .get(&node_id)
                        .cloned()
                        .unwrap_or_else(|| wfr.intent.prompt.to_string());
                    let deadline = if wfr.intent.deadline_ns > 0 {
                        wfr.intent.deadline_ns
                    } else {
                        now_ns() + self.cfg.default_task_deadline_ns
                    };
                    (prompt, spec.priority, deadline)
                };
                self.publish_task(&task_id, &prompt, priority, deadline)
                    .await?;
                self.publish_state(&wf, 3).await?;
                published += 1;
            }
        }
        Ok(published)
    }

    /// Tick do watcher: tasks terminais → on_done/on_failed.
    pub async fn tick_watch_tasks(&mut self) -> Result<usize, RuntimeError> {
        let mut handled = 0;
        let all = self.dataspace.all_tasks().await?;
        for task in all {
            let task_id = task.task_id.to_string();
            let Some((wf, node)) = self.task_index.get(&task_id).cloned() else {
                continue;
            };
            let attempt = {
                let Some(wfr) = self.workflows.get(&wf) else {
                    continue;
                };
                wfr.state
                    .nodes
                    .get(&node)
                    .map(|p| p.active_attempt)
                    .unwrap_or(1)
            };
            let Some(wfr) = self.workflows.get_mut(&wf) else {
                continue;
            };
            if wfr.terminal {
                continue;
            }
            match task.status {
                TS_DONE => {
                    let outputs = self.dataspace.read_task_outputs(&task_id).await?;
                    let mut content = String::new();
                    for o in &outputs {
                        if o.is_final {
                            content.push_str(&o.content);
                        }
                    }
                    let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
                    let out_name = node_output_name(&wfr.graph, &node);
                    match wfr.state.on_done(
                        &self.journal,
                        &wfr.graph,
                        &node,
                        attempt,
                        &[(out_name, hash)],
                    ) {
                        Ok(_) => {
                            handled += 1;
                            let all_done = wfr.state.nodes.values().all(|p| p.done);
                            if all_done {
                                wfr.terminal = true;
                                self.publish_state(&wf, 4).await?;
                            } else {
                                self.publish_state(&wf, 3).await?;
                            }
                        }
                        Err(crate::engine::EngineError::AlreadyDone(_))
                        | Err(crate::engine::EngineError::StaleAttempt { .. }) => {}
                        Err(_) => {}
                    }
                }
                TS_FAILED => {
                    let failover = wfr.failover;
                    match wfr
                        .state
                        .on_failed(&self.journal, &wfr.graph, &node, attempt, failover)
                    {
                        Ok(Some(_)) => {
                            handled += 1;
                            self.task_index.remove(&task.task_id); // nova tentativa = novo id
                            self.publish_state(&wf, 3).await?;
                        }
                        Ok(None) => {
                            wfr.terminal = true;
                            wfr.failed_reason = Some(format!("nó {node} falhou sem retry"));
                            self.publish_state(&wf, 5).await?;
                            handled += 1;
                        }
                        Err(_) => {}
                    }
                }
                _ => {}
            }
        }
        Ok(handled)
    }

    /// Sweeper: tentativas que não terminaram antes do prazo → IN_DOUBT
    /// (nunca re-tenta efeito incerto automaticamente).
    pub async fn tick_timeouts(&mut self) -> Result<usize, RuntimeError> {
        let now = now_ns();
        let all = self.dataspace.all_tasks().await?;
        let mut marked = 0;
        for task in all {
            if task.status == TS_DONE || task.status == TS_FAILED {
                continue;
            }
            if task.deadline_ns > 0 && now > task.deadline_ns {
                self.journal.mark_in_doubt(&task.task_id.to_string())?;
                marked += 1;
            }
        }
        Ok(marked)
    }

    pub fn workflow_done(&self, wf: &str) -> Option<bool> {
        self.workflows
            .get(wf)
            .map(|w| w.terminal && w.failed_reason.is_none())
    }

    pub fn workflows_summary(&self) -> Vec<(String, bool, Option<String>)> {
        self.workflows
            .iter()
            .map(|(k, w)| (k.clone(), w.terminal, w.failed_reason.clone()))
            .collect()
    }

    // ---------- helpers ----------

    fn pending_intent_for(&self, request_id: &str) -> Option<String> {
        self.workflows
            .iter()
            .find(|(_, w)| w.intent.request_id.to_string() == request_id)
            .map(|(k, _)| k.clone())
    }

    async fn publish_task(
        &self,
        task_id: &str,
        prompt: &str,
        priority: i32,
        deadline_ns: u64,
    ) -> Result<(), RuntimeError> {
        let messages = format!(
            "[{{\"role\":\"user\",\"content\":{}}}]",
            serde_json::json!(prompt)
        );
        let task = Task {
            task_id: task_id.to_string(),
            client_id: "wf-engine".to_string(),
            assigned_agent: String::new(),
            target_agent: String::new(),
            model_required: 0,
            model_name: String::new(),
            messages_json: messages,
            temperature: 0.1,
            max_tokens: 256,
            stream: false,
            status: 0, // PENDING
            priority,
            created_at_ns: now_ns(),
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns,
            retry_count: 0,
            finish_reason: String::new(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        };
        self.dataspace.write_task(task).await?;
        Ok(())
    }

    async fn publish_plan(&self, wf: &str, candidate: &PlanCandidate) -> Result<(), RuntimeError> {
        let wfr = &self.workflows[wf];
        let nodes: Vec<w::PlanNode> = candidate
            .nodes
            .iter()
            .map(|n| w::PlanNode {
                node_id: to_s(n.node_id.clone()),
                kind: 0,
                required_capabilities: seq_n::<_, 16>(
                    n.required_capabilities
                        .iter()
                        .map(|c| to_s(c.clone()))
                        .collect(),
                ),
                model_id: to_s(n.model_id.clone()),
                target_agent_id: to_s(n.target_agent_id.clone()),
                prompt_template: to_s(n.prompt_template.clone()),
                dependencies: seq_n::<_, 32>(
                    n.dependencies.iter().map(|d| to_s(d.clone())).collect(),
                ),
                inputs: seq_n::<_, 16>(vec![]),
                output_names: seq_n::<_, 8>(
                    n.output_names.iter().map(|o| to_s(o.clone())).collect(),
                ),
                max_attempts: n.max_attempts as u16,
                deadline_ns: n.deadline_ns,
                max_tokens: n.max_tokens as u32,
                max_output_bytes: n.max_output_bytes as u32,
                security_context_ref: to_s(String::new()),
                priority: n.priority,
                side_effect_key: to_s(n.side_effect_key.clone()),
            })
            .collect();
        let plan = w::WorkflowPlan {
            workflow_id: to_s(wf.to_string()),
            plan_revision: 1,
            schema_version: 1,
            request_id: to_s(wfr.intent.request_id.to_string()),
            intent_revision: wfr.intent.intent_revision,
            effective_decision_id: to_s(format!("dec-{}", wfr.intent.request_id)),
            policy_revision: wfr.intent.policy_revision,
            capability_catalog_revision: wfr.intent.capability_catalog_revision,
            planner_id: to_s(candidate.planner_id.clone()),
            planner_revision: to_s(candidate.planner_revision.clone()),
            created_at_ns: now_ns(),
            deadline_ns: wfr.intent.deadline_ns,
            qos_profile: 0,
            nodes: seq_n::<_, 32>(nodes),
            plan_hash: to_s(String::new()),
        };
        self.wspace.write_plan(plan).await?;
        Ok(())
    }

    async fn publish_state(&mut self, wf: &str, status: i32) -> Result<(), RuntimeError> {
        let state_rev = {
            let wfr = self.workflows.get_mut(wf).expect("wf");
            wfr.state_rev += 1;
            wfr.state_rev
        };
        let wfr = &self.workflows[wf];
        let failed = wfr.failed_reason.is_some();
        let nodes: Vec<w::NodeState> = wfr
            .graph
            .nodes
            .iter()
            .map(|n| {
                let p = wfr.state.nodes.get(&n.node_id);
                w::NodeState {
                    node_id: to_s(n.node_id.clone()),
                    status: match p.map(|p| (p.done, p.failed)) {
                        Some((true, _)) => 5,     // DONE
                        Some((false, true)) => 6, // FAILED
                        // PUBLISHED: o task_id laya1- só casa depois de
                        // materializada — barreira do agente exige
                        // READY/PUBLISHED para claim.
                        _ => 2,
                    },
                    attempt_id: p.map(|p| p.active_attempt).unwrap_or(0),
                    task_id: to_s(String::new()),
                    assigned_agent_id: to_s(String::new()),
                    ready: true,
                    started_at_ns: 0,
                    completed_at_ns: 0,
                    outputs: seq_n::<_, 8>(vec![]),
                    reason_code: if failed { 11 } else { 0 },
                }
            })
            .collect();
        let state = w::WorkflowState {
            workflow_id: to_s(wf.to_string()),
            plan_revision: wfr.graph.plan_revision,
            schema_version: 1,
            state_revision: state_rev,
            request_id: to_s(wfr.intent.request_id.to_string()),
            effective_decision_id: to_s(String::new()),
            plan_hash: to_s(String::new()),
            status,
            reason_code: if failed { 11 } else { 0 },
            nodes: seq_n::<_, 32>(nodes),
            created_at_ns: now_ns(),
            updated_at_ns: now_ns(),
            expires_at_ns: wfr.intent.deadline_ns,
            cancelled_by_request_id: to_s(String::new()),
            effective_qos_config_hash: to_s(String::new()),
            fallback_template_id: to_s(String::new()),
        };
        self.wspace.write_state(state).await?;
        Ok(())
    }
}

fn node_output_name(graph: &PlanGraph, node: &str) -> String {
    graph
        .node(node)
        .and_then(|n| n.output_names.first().cloned())
        .unwrap_or_else(|| "out".into())
}

fn verdict_name(v: &Verdict) -> String {
    match v {
        Verdict::Accept { .. } => "accept".into(),
        Verdict::Error { .. } => "error".into(),
        Verdict::Reject { .. } => "reject".into(),
    }
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
