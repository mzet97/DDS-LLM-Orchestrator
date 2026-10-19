//! Barreira adaptativa de claim (T-900-M3-02; REQ-900-006/007; ADR-003).
//!
//! Modo OPT-IN (`--adaptive`): tasks físicas da fase 900 — prefixo
//! `laya1-` no `task_id` — só podem ser reivindicadas quando:
//! 1. o `task_id` corresponde EXATAMENTE ao ID determinístico de um nó de um
//!    **Workflow.Plan** válido publicado, para a **tentativa ativa** registrada
//!    no **Workflow.State** (barreira Plan+State+Task — ordem de chegada
//!    irrelevante: sem as três peças, não executa — V06);
//! 2. o nó está em estado materializável (READY/PUBLISHED no State);
//! 3. o agente é elegível pela M3-01 (capabilities/registry/modelo) —
//!    REVALIDADO antes e depois do readback (janela de revogação);
//! 4. `target_agent` (task e nó) exigem IGUALDADE EXATA — no modo adaptativo
//!    NÃO há prefix matching (o prefixo vazio do legado não autoriza
//!    instância errada — correção do achado A04);
//! 5. o prazo do nó/plano não venceu.
//!
//! Tasks SEM prefixo `laya1-` seguem o caminho LEGADO intocado (prefix
//! matching, EXP1b/EXP4 preservados — REQ-900-012). Sem `--adaptive`, nada
//! aqui é consultado.
//!
//! Convenção V1 do ID físico (documental; não é o payload_hash):
//! `task_id = "laya1-" + hex(sha256("{workflow_id}|{plan_revision}|{node_id}|{attempt}"))`.

use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_contract::workflow::dds_llm_workflow_v1::{AgentCapabilities, WorkflowPlan, WorkflowState};
use std::collections::HashMap;
use workflow_engine::eligibility::is_eligible;
use workflow_engine::NotEligible as EligibilityError;

/// Status de NodeStatus materializáveis para claim (ordinais: READY=1, PUBLISHED=2).
const CLAIMABLE_NODE_STATUS: [i32; 2] = [1, 2];

pub const LAYA1_PREFIX: &str = "laya1-";

/// ID físico determinístico por tentativa (fonte única: workflow-engine).
pub use workflow_engine::task_id::laya1_task_id;

/// Por que a barreira bloqueou (V05/V06).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Blocked {
    #[error("elegibilidade: {0}")]
    NotEligible(#[from] EligibilityError),
    #[error("task laya1 sem nó correspondente em plan/state (barreira incompleta)")]
    UnresolvedNode,
    #[error("nó {node_id} não está READY/PUBLISHED (status {status})")]
    NodeNotClaimable { node_id: String, status: i32 },
    #[error("tentativa {got} != ativa {active}")]
    AttemptMismatch { got: u32, active: u32 },
    #[error("target_agent '{task_target}' exige igualdade exata (agente '{agent_id}')")]
    TargetMismatch {
        task_target: String,
        agent_id: String,
    },
    #[error("nó com target '{0}' reservado a outro agente")]
    NodeTargeted(String),
    #[error("prazo do nó/plano vencido")]
    DeadlinePassed,
}

/// Decisão da barreira para uma task.
#[derive(Debug, Clone, PartialEq)]
pub enum AdaptiveDecision {
    /// Task legada (sem `laya1-`): siga as regras clássicas.
    Legacy,
    /// Pode tentar o claim (nas duas checagens: pré-write e pós-readback).
    Proceed,
    /// Bloqueada com causa tipada.
    Block(Blocked),
}

/// Foto dos tópicos da fase 900 relevante para a barreira.
#[derive(Debug, Default, Clone)]
pub struct AdaptiveSnapshot {
    /// workflow_id -> planos por revisão (última revisão vence).
    pub plans: HashMap<String, WorkflowPlan>,
    /// workflow_id -> estado por (plan_revision ativa).
    pub states: HashMap<String, WorkflowState>,
    /// agent_id -> anúncio vigente.
    pub capabilities: HashMap<String, AgentCapabilities>,
}

impl AdaptiveSnapshot {
    pub fn upsert_plan(&mut self, p: WorkflowPlan) {
        let wf = p.workflow_id.to_string();
        match self.plans.get_mut(&wf) {
            Some(cur) if cur.plan_revision > p.plan_revision => {}
            _ => {
                self.plans.insert(wf, p);
            }
        }
    }

    pub fn upsert_state(&mut self, s: WorkflowState) {
        let wf = s.workflow_id.to_string();
        match self.states.get_mut(&wf) {
            Some(cur) if cur.plan_revision > s.plan_revision => {}
            _ => {
                self.states.insert(wf, s);
            }
        }
    }

    pub fn upsert_capabilities(&mut self, c: AgentCapabilities) {
        let id = c.agent_id.to_string();
        match self.capabilities.get_mut(&id) {
            Some(cur) if cur.announcement_revision > c.announcement_revision => {}
            _ => {
                self.capabilities.insert(id, c);
            }
        }
    }
}

/// Resolve (plano, nó, tentativa ativa) para um task_id laya1.
struct Resolved<'a> {
    plan: &'a WorkflowPlan,
    node_index: usize,
    node_status: i32,
}

fn resolve<'a>(snap: &'a AdaptiveSnapshot, task_id: &str, now_ns: u64) -> Option<Resolved<'a>> {
    for plan in snap.plans.values() {
        let wf = plan.workflow_id.to_string();
        let Some(state) = snap.states.get(&wf) else {
            continue;
        };
        if state.plan_revision != plan.plan_revision {
            continue; // estado de outra revisão: barreira incompleta
        }
        if plan.deadline_ns > 0 && plan.deadline_ns <= now_ns {
            continue;
        }
        for (i, node) in plan.nodes.iter().enumerate() {
            let Some(ns) = state.nodes.iter().find(|n| n.node_id == node.node_id) else {
                continue;
            };
            // O task_id determinístico da TENTATIVA ATIVA: tentativa velha
            // (saída tardia de tentativa anterior) simplesmente não casa.
            if laya1_task_id(
                &wf,
                plan.plan_revision,
                &node.node_id.to_string(),
                ns.attempt_id,
            ) == task_id
            {
                return Some(Resolved {
                    plan,
                    node_index: i,
                    node_status: ns.status,
                });
            }
        }
    }
    None
}

/// Checagem pura da barreira (testável com fixtures; V05/V06).
///
/// `registry` é o AgentState DO PRÓPRIO agente (autoridade operacional);
/// `announcement` é o anúncio do próprio agente (self) — a barreira é sobre o
/// QUEM reivindica, não sobre terceiros.
#[allow(clippy::too_many_arguments)]
pub fn barrier_check(
    snap: &AdaptiveSnapshot,
    task: &Task,
    agent_id: &str,
    announcement: Option<&AgentCapabilities>,
    registry: Option<&dds_contract::generated::dds_llm_orchestrator::AgentState>,
    target_agent_prefix_configured: &str,
    now_ns: u64,
) -> AdaptiveDecision {
    if !task.task_id.starts_with(LAYA1_PREFIX) {
        return AdaptiveDecision::Legacy;
    }
    let Some(r) = resolve(snap, &task.task_id, now_ns) else {
        return AdaptiveDecision::Block(Blocked::UnresolvedNode);
    };
    let nodes = r.plan.nodes.as_slice();
    let node = &nodes[r.node_index];

    if !CLAIMABLE_NODE_STATUS.contains(&r.node_status) {
        return AdaptiveDecision::Block(Blocked::NodeNotClaimable {
            node_id: node.node_id.to_string(),
            status: r.node_status,
        });
    }

    // Target exato: task E nó (o prefixo configurado do legado NÃO autoriza
    // instância errada no modo adaptativo — V05/achado A04).
    let _ = target_agent_prefix_configured; // deliberadamente ignorado no modo adaptativo
    if !task.target_agent.is_empty() && task.target_agent != agent_id {
        return AdaptiveDecision::Block(Blocked::TargetMismatch {
            task_target: task.target_agent.clone(),
            agent_id: agent_id.to_string(),
        });
    }
    let node_target = node.target_agent_id.to_string();
    if !node_target.is_empty() && node_target != agent_id {
        return AdaptiveDecision::Block(Blocked::NodeTargeted(node_target));
    }

    if node.deadline_ns > 0 && node.deadline_ns <= now_ns {
        return AdaptiveDecision::Block(Blocked::DeadlinePassed);
    }

    let required: Vec<String> = node
        .required_capabilities
        .iter()
        .map(|c| c.to_string())
        .collect();
    let model = node.model_id.to_string();
    let model_opt = if model.is_empty() {
        None
    } else {
        Some(model.as_str())
    };
    match is_eligible(announcement, registry, &required, model_opt, now_ns) {
        Ok(()) => AdaptiveDecision::Proceed,
        Err(e) => AdaptiveDecision::Block(Blocked::NotEligible(e)),
    }
}

/// Runtime da barreira: snapshot concorrente alimentado pelo WorkflowSpace
/// (DDS) ou por fixtures (testes); consultado ANTES e DEPOIS do readback.
#[derive(Debug, Default)]
pub struct AdaptiveBarrier {
    snapshot: std::sync::RwLock<AdaptiveSnapshot>,
    agent_id: String,
}

impl AdaptiveBarrier {
    pub fn new(agent_id: &str) -> Self {
        Self {
            snapshot: std::sync::RwLock::new(AdaptiveSnapshot::default()),
            agent_id: agent_id.to_string(),
        }
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn upsert_plan(&self, p: WorkflowPlan) {
        self.snapshot.write().expect("rwlock").upsert_plan(p);
    }

    pub fn upsert_state(&self, s: WorkflowState) {
        self.snapshot.write().expect("rwlock").upsert_state(s);
    }

    pub fn upsert_capabilities(&self, c: AgentCapabilities) {
        self.snapshot
            .write()
            .expect("rwlock")
            .upsert_capabilities(c);
    }

    /// Checagem de claim (usa o anúncio do PRÓPRIO agente).
    pub fn check(
        &self,
        task: &Task,
        registry: Option<&dds_contract::generated::dds_llm_orchestrator::AgentState>,
        target_prefix_configured: &str,
        now_ns: u64,
    ) -> AdaptiveDecision {
        let snap = self.snapshot.read().expect("rwlock").clone();
        // O anúncio relevante é o do PRÓPRIO agente (self) — a barreira é
        // sobre quem reivindica.
        let announcement = snap.capabilities.get(&self.agent_id);
        barrier_check(
            &snap,
            task,
            &self.agent_id,
            announcement,
            registry,
            target_prefix_configured,
            now_ns,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laya1_id_is_deterministic_and_prefixed() {
        let a = laya1_task_id("wf", 1, "node-a", 1);
        let b = laya1_task_id("wf", 1, "node-a", 1);
        assert_eq!(a, b);
        assert!(a.starts_with("laya1-"));
        assert_eq!(a.len(), 6 + 64);
        assert_ne!(a, laya1_task_id("wf", 1, "node-a", 2));
    }
}

/// Alimentador DDS + auto-anúncio (feature `dds`; T-900-M3-02/03).
///
/// Subscreve `Workflow.Plan/State` + `Agent.Capabilities` no domínio
/// adaptativo e alimenta a barreira; publica o ANÚNCIO do próprio agente
/// (heartbeat 5 s; expires 10 s — contracts.md). Anúncio não concede
/// permissão (D10).
#[cfg(feature = "dds")]
pub mod dds_feeder {

    use super::AdaptiveBarrier;
    use dds_contract::workflow::dds_llm_workflow_v1::AgentCapabilities;
    use dds_dataspace::workflow_space::{
        dds_impl::DdsWorkflowSpace, WorkflowSpaceApi, WorkflowSpaceLimits,
    };
    use futures_util::StreamExt;
    use std::sync::Arc;
    use std::time::Duration;
    use workflow_engine::capability_catalog::catalog_v1;

    /// Sobe o WorkflowSpace no domínio adaptativo e devolve handles.
    pub fn mount(domain: u32) -> anyhow::Result<DdsWorkflowSpace> {
        DdsWorkflowSpace::new(domain, WorkflowSpaceLimits::default())
            .map_err(|e| anyhow::anyhow!("DdsWorkflowSpace: {e}"))
    }

    /// Spawn das tasks de alimentação (planos/estados/capabilities -> barreira).
    pub async fn spawn_feeders(space: Arc<DdsWorkflowSpace>, barrier: Arc<AdaptiveBarrier>) {
        let mut plans = space.subscribe_plans();
        let bp = Arc::clone(&barrier);
        tokio::spawn(async move {
            while let Some(p) = plans.next().await {
                bp.upsert_plan((*p).clone());
            }
        });
        let mut states = space.subscribe_states();
        let bs = Arc::clone(&barrier);
        tokio::spawn(async move {
            while let Some(s) = states.next().await {
                bs.upsert_state((*s).clone());
            }
        });
        let mut caps = space.subscribe_capabilities();
        let bc = Arc::clone(&barrier);
        tokio::spawn(async move {
            while let Some(c) = caps.next().await {
                bc.upsert_capabilities((*c).clone());
            }
        });
    }

    /// Anúncio periódico do próprio agente (Agent.Capabilities).
    pub async fn announce_self(
        space: Arc<DdsWorkflowSpace>,
        agent_id: String,
        capabilities: Vec<String>,
        catalog_revision: u32,
    ) -> anyhow::Result<()> {
        let catalog = catalog_v1();
        let unknown = catalog.unknown(capabilities.iter().map(|s| s.as_str()));
        anyhow::ensure!(
            unknown.is_empty(),
            "capabilities fora do catálogo rev {}: {unknown:?}",
            catalog.revision
        );
        anyhow::ensure!(
            catalog_revision == catalog.revision,
            "catálogo pinado {catalog_revision} != ativo {}",
            catalog.revision
        );
        let mut revision: u32 = 0;
        loop {
            revision = revision.saturating_add(1);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let announcement = AgentCapabilities {
                agent_id: agent_id.clone().try_into()?,
                schema_version: 1,
                announcement_revision: revision,
                capability_catalog_revision: catalog_revision,
                capabilities: cyclonedds::DdsBoundedSequence::from_vec(
                    capabilities
                        .iter()
                        .map(|c| c.as_str().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )?,
                model_ids: cyclonedds::DdsBoundedSequence::from_vec(vec![])?,
                tool_versions: cyclonedds::DdsBoundedSequence::from_vec(vec![])?,
                adaptive_protocol_version: 1,
                security_context_ref: String::new().try_into()?,
                created_at_ns: now,
                valid_until_ns: now + 10_000_000_000,
            };
            if let Err(e) = space.write_capabilities(announcement).await {
                tracing::warn!(error = %e, "anúncio Agent.Capabilities falhou");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
