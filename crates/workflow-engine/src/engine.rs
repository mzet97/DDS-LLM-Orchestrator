//! Scheduler/materializer/recovery do engine (T-900-M5-03/04; REQ-900-010).
//!
//! Núcleo de domínio PURO sobre o [`Journal`]: readiness por DEPENDÊNCIA
//! (nunca por ordem de chegada — joins só com outputs validados),
//! materialização com task_id físico por tentativa, retomada com tentativa
//! única, retry (novo attempt/novo task_id; FAILOVER habilita) e resultados
//! tardios de tentativa antiga IGNORADOS. A publicação DDS sai pelo outbox.

use crate::dag::PlanCandidate;
use crate::journal::{Journal, JournalError};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeSpec {
    pub node_id: String,
    pub dependencies: Vec<String>,
    pub output_names: Vec<String>,
    pub max_attempts: u32,
    pub max_tokens: u64,
    pub priority: i32,
    /// P1 (review PR#10): parâmetros de execução declarados no plano —
    /// materializados na Task pelo scheduler (vazio = legado, sem fixação).
    pub model_id: String,
    pub target_agent_id: String,
}

impl NodeSpec {
    pub fn from_candidate(c: &crate::dag::CandidateNode) -> Self {
        Self {
            node_id: c.node_id.clone(),
            dependencies: c.dependencies.clone(),
            output_names: c.output_names.clone(),
            max_attempts: c.max_attempts.max(1),
            max_tokens: c.max_tokens,
            priority: c.priority,
            model_id: c.model_id.clone(),
            target_agent_id: c.target_agent_id.clone(),
        }
    }
}

pub struct PlanGraph {
    pub workflow_id: String,
    pub plan_revision: u32,
    pub nodes: Vec<NodeSpec>,
}

impl PlanGraph {
    pub fn from_candidate(workflow_id: &str, plan_revision: u32, c: &PlanCandidate) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            plan_revision,
            nodes: c.nodes.iter().map(NodeSpec::from_candidate).collect(),
        }
    }

    pub fn node(&self, id: &str) -> Option<&NodeSpec> {
        self.nodes.iter().find(|n| n.node_id == id)
    }

    /// Nós PRONTOS: todas as dependências DONE (validadas) e o nó ainda não
    /// concluído/falho. Join depende do conjunto, não da ordem de chegada.
    pub fn ready_nodes(&self, done: &HashSet<String>) -> Vec<&NodeSpec> {
        self.nodes
            .iter()
            .filter(|n| {
                !done.contains(&n.node_id) && n.dependencies.iter().all(|d| done.contains(d))
            })
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("journal: {0}")]
    Journal(#[from] JournalError),
    #[error("nó {0} desconhecido no plano")]
    UnknownNode(String),
    #[error("output inválido do nó {0}: hash vazio ou tamanho > declarado")]
    InvalidOutput(String),
    #[error("resultado de tentativa antiga ({node}@{attempt}) ignorado: ativa é {active}")]
    StaleAttempt {
        node: String,
        attempt: u32,
        active: u32,
    },
    #[error("nó {0} já concluído (duplicata ignorada)")]
    AlreadyDone(String),
    #[error(
        "tentativa {1} do nó {0} com reserva consumida/IN_DOUBT: re-execução automática bloqueada"
    )]
    ReservationConsumed(String, u32),
}

/// Estado por nó mantido EM MEMÓRIA pelo engine (derivável do journal após
/// restart — recovery reconstitui via `attempts`).
#[derive(Debug, Default, Clone)]
pub struct NodeProgress {
    pub active_attempt: u32,
    pub done: bool,
    pub failed: bool,
    /// P1 (review PR#10): tentativa ativa com desfecho incerto (IN_DOUBT) —
    /// bloqueada p/ re-execução automática (nem materializa nem agenda).
    pub in_doubt: bool,
    pub outputs: Vec<(String, String)>, // (nome, content_hash)
}

#[derive(Debug, Default)]
pub struct EngineState {
    pub nodes: HashMap<String, NodeProgress>,
}

impl EngineState {
    /// Recovery: reconstrói estado ativo a partir do journal (após restart).
    pub fn recover(journal: &Journal, graph: &PlanGraph) -> Result<Self, EngineError> {
        let mut state = Self::default();
        for n in &graph.nodes {
            let mut progress = NodeProgress {
                active_attempt: 1,
                ..Default::default()
            };
            for attempt in 1..=n.max_attempts {
                if let Ok(row) = journal.attempt_state(&graph.workflow_id, &n.node_id, attempt) {
                    match row.as_str() {
                        "done" => {
                            progress.done = true;
                            progress.active_attempt = attempt;
                            break;
                        }
                        "failed" | "cancelled" => {
                            progress.failed = true;
                            progress.active_attempt = attempt;
                            break;
                        }
                        "in_doubt" => {
                            // P1 (review PR#10): efeito com desfecho incerto
                            // NÃO é re-agendado no recovery — o nó fica
                            // bloqueado (resolução manual/monitor).
                            progress.active_attempt = attempt;
                            progress.in_doubt = true;
                        }
                        other => {
                            progress.active_attempt = attempt;
                            let _ = other;
                        }
                    }
                }
            }
            state.nodes.insert(n.node_id.clone(), progress);
        }
        Ok(state)
    }

    pub fn done_set(&self) -> HashSet<String> {
        self.nodes
            .iter()
            .filter(|(_, p)| p.done)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Materializa a tentativa ATIVA de um nó: task_id físico + registro no
    /// journal + outbox (a publicação real drena o outbox — at-least-once).
    pub fn materialize(
        &mut self,
        journal: &Journal,
        graph: &PlanGraph,
        node_id: &str,
        budget_microunits_per_attempt: u64,
    ) -> Result<String, EngineError> {
        let spec = graph
            .node(node_id)
            .ok_or_else(|| EngineError::UnknownNode(node_id.into()))?;
        let progress = self.nodes.entry(node_id.to_string()).or_default();
        if progress.done {
            return Err(EngineError::AlreadyDone(node_id.into()));
        }
        if progress.in_doubt {
            return Err(EngineError::ReservationConsumed(
                node_id.into(),
                progress.active_attempt,
            ));
        }
        let attempt = progress.active_attempt.max(1);
        progress.active_attempt = attempt;

        let task_id = crate::task_id::laya1_task_id(
            &graph.workflow_id,
            graph.plan_revision,
            node_id,
            attempt,
        );
        // Reserva idempotente por tentativa: replay não duplica; ERRO na
        // reserva RECUSA a publicação (fail-closed — contracts).
        // P1 (review PR#10): `AlreadyConsumed` (reserva consumida ou IN_DOUBT)
        // RECUSA a materialização — antes o outcome era ignorado e o mesmo
        // efeito incerto era re-publicado (no recovery pós-restart).
        let key = format!("attempt:{}:{}:{}", graph.workflow_id, node_id, attempt);
        match journal.reserve(
            &graph.workflow_id,
            &key,
            &key,
            spec.max_tokens,
            budget_microunits_per_attempt,
        )? {
            crate::journal::ReserveOutcome::Reserved { .. }
            | crate::journal::ReserveOutcome::Replayed { .. } => {}
            crate::journal::ReserveOutcome::AlreadyConsumed => {
                // A tentativa já foi consumida OU marcada IN_DOUBT: efeito
                // com desfecho incerto NÃO se re-executa automaticamente.
                progress.in_doubt = true;
                return Err(EngineError::ReservationConsumed(node_id.into(), attempt));
            }
        }
        journal.record_attempt(&graph.workflow_id, node_id, attempt, &task_id, "published")?;
        journal.outbox_append(
            &|_| Ok(()),
            &graph.workflow_id,
            "publish_task",
            &format!(r#"{{"task_id":"{task_id}","node":"{node_id}","attempt":{attempt}}}"#),
        )?;
        Ok(task_id)
    }

    /// Resultado DONE de uma tentativa: valida output, conclui o nó e
    /// devolve os nós que ficaram PRONTOS (joins disparados por conjunto).
    /// Resultado de tentativa != ativa → `StaleAttempt` (ignorado pelo
    /// chamador — resultado antigo não conclui tentativa nova; V09).
    pub fn on_done(
        &mut self,
        journal: &Journal,
        graph: &PlanGraph,
        node_id: &str,
        attempt: u32,
        outputs: &[(String, String)],
    ) -> Result<Vec<String>, EngineError> {
        let spec = graph
            .node(node_id)
            .ok_or_else(|| EngineError::UnknownNode(node_id.into()))?;
        let progress = self.nodes.get_mut(node_id).expect("materialized");
        if progress.done {
            return Err(EngineError::AlreadyDone(node_id.into()));
        }
        if attempt != progress.active_attempt {
            return Err(EngineError::StaleAttempt {
                node: node_id.into(),
                attempt,
                active: progress.active_attempt,
            });
        }
        // Output validado: todo nome declarado presente, hash não vazio.
        for name in &spec.output_names {
            match outputs.iter().find(|(n, _)| n == name) {
                Some((_, h)) if !h.is_empty() => {}
                _ => return Err(EngineError::InvalidOutput(node_id.into())),
            }
        }
        progress.done = true;
        progress.outputs = outputs.to_vec();
        journal.record_attempt(
            &graph.workflow_id,
            node_id,
            attempt,
            &crate::task_id::laya1_task_id(
                &graph.workflow_id,
                graph.plan_revision,
                node_id,
                attempt,
            ),
            "done",
        )?;
        journal.outbox_append(
            &|_| Ok(()),
            &graph.workflow_id,
            "publish_state",
            &format!(r#"{{"node":"{node_id}","status":"done"}}"#),
        )?;
        // Libera a reserva da tentativa com medida simples (tokens saíram do
        // escopo aqui; consumo incerto é tratado no timeout path).
        let key = format!("attempt:{}:{}:{}", graph.workflow_id, node_id, attempt);
        let _ = journal.release_reservation(&key, None);

        let done = self.done_set();
        Ok(graph
            .ready_nodes(&done)
            .iter()
            .map(|n| n.node_id.clone())
            .collect())
    }

    /// Falha de tentativa: retry DENTRO de max_attempts (novo attempt, novo
    /// task_id físico — tentativa antiga fica fechada) ou nó FAILED.
    /// `failover_enabled` (objetivo FAILOVER) habilita retry; sem ele, uma
    /// falha em attempt 1 de max 3 NÃO re-tenta (contrato V1 do mapper).
    pub fn on_failed(
        &mut self,
        journal: &Journal,
        graph: &PlanGraph,
        node_id: &str,
        attempt: u32,
        failover_enabled: bool,
    ) -> Result<Option<u32>, EngineError> {
        let spec = graph
            .node(node_id)
            .ok_or_else(|| EngineError::UnknownNode(node_id.into()))?;
        let progress = self.nodes.get_mut(node_id).expect("materialized");
        if progress.done {
            return Err(EngineError::AlreadyDone(node_id.into()));
        }
        if attempt != progress.active_attempt {
            return Err(EngineError::StaleAttempt {
                node: node_id.into(),
                attempt,
                active: progress.active_attempt,
            });
        }
        // consumo INCERTO da tentativa falha: cobra a reserva integral
        let key = format!("attempt:{}:{}:{}", graph.workflow_id, node_id, attempt);
        let _ = journal.release_reservation(&key, None);
        journal.record_attempt(
            &graph.workflow_id,
            node_id,
            attempt,
            &crate::task_id::laya1_task_id(
                &graph.workflow_id,
                graph.plan_revision,
                node_id,
                attempt,
            ),
            "failed",
        )?;
        if failover_enabled && attempt < spec.max_attempts {
            let next = attempt + 1;
            progress.active_attempt = next;
            journal.outbox_append(
                &|_| Ok(()),
                &graph.workflow_id,
                "publish_state",
                &format!(r#"{{"node":"{node_id}","retry":{next}}}"#),
            )?;
            Ok(Some(next))
        } else {
            progress.failed = true;
            Ok(None)
        }
    }
}

/// Planner adapter (D7: planner LLM é RUST via inferência existente; a
/// seam é a trait — implementação LLM real é integrada ao caminho DDS de
/// inferência no gate de integração; template direto NÃO chama planner).
pub trait Planner: Send + Sync {
    fn plan(&self, intent_plan_input: &PlanRequest) -> Result<PlanCandidate, String>;
}

#[derive(Debug, Clone)]
pub struct PlanRequest {
    pub workflow_id: String,
    pub prompt: String,
}

/// Template direto (planner_id = static-template): um nó, sem chamar LLM.
pub struct DirectTemplatePlanner;

impl Planner for DirectTemplatePlanner {
    fn plan(&self, input: &PlanRequest) -> Result<PlanCandidate, String> {
        Ok(PlanCandidate {
            planner_id: "static-template".into(),
            planner_revision: "direct-v1".into(),
            planner_tokens: 0,
            nodes: vec![crate::dag::CandidateNode {
                node_id: "direct-0".into(),
                kind: crate::dag::NodeKindCand::Inference,
                required_capabilities: vec!["summarization".into()],
                model_id: String::new(),
                target_agent_id: String::new(),
                prompt_template: input.prompt.clone(),
                dependencies: vec![],
                output_names: vec!["out".into()],
                inputs: vec![],
                max_attempts: 2,
                max_tokens: 512,
                max_output_bytes: 65536,
                deadline_ns: 0,
                priority: 5,
                side_effect_key: String::new(),
            }],
        })
    }
}
