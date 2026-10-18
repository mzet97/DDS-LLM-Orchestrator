//! WorkflowSpace — acesso aos 5 tópicos da fase 900 por COMPOSIÇÃO
//! (T-900-M1-03; REQ-900-003/008/014).
//!
//! Trait própria ([`WorkflowSpaceApi`]) em vez de esticar a `DataSpaceApi`
//! global: os mocks e consumidores dos 19 tópicos canônicos não mudam
//! (Prompt Master §6, "trait nova/composição limitada").
//!
//! O backend [`InMemoryWorkflowSpace`] aplica validação de contrato,
//! KeepLast(1) por chave, limites e snapshot/restore para replay; é o
//! caminho de teste dos marcos M2/M3/M5 sem DDS.
//!
//! O backend [`DdsWorkflowSpace`] (feature `dds`) cria entidades reais com
//! os perfis `qos::profiles::workflow`. Erros TIPADOS no aceite (capacidade
//! cheia, versão inválida, lag): nunca truncar nem descartar silenciosamente
//! (R-BOUNDS).

use dds_contract::workflow::dds_llm_workflow_v1::{
    AgentCapabilities, RoutingDecision, WorkflowIntent, WorkflowPlan, WorkflowState,
};
use futures_core::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

/// Versão de schema aceita em V1 (emenda 2: validada por contrato, não IDL).
pub const SCHEMA_VERSION_V1: u16 = 1;

/// Limites por tópico (contracts.md §"QoS por entidade"; proposta, não medição).
#[derive(Debug, Clone, Copy)]
pub struct WorkflowSpaceLimits {
    /// Máximo de instâncias (chaves) simultâneas por tópico.
    pub max_instances_intent: usize,
    pub max_instances_decision: usize,
    pub max_instances_capabilities: usize,
    pub max_instances_plan: usize,
    pub max_instances_state: usize,
    /// Orçamento de bytes do cache de aplicação (todos os tópicos somados;
    /// estimativa por amostra — REFLETE o teto de 64 MiB do contracts.md).
    pub cache_byte_budget: usize,
}

impl Default for WorkflowSpaceLimits {
    fn default() -> Self {
        Self {
            max_instances_intent: 64,
            max_instances_decision: 256,
            max_instances_capabilities: 256,
            max_instances_plan: 32,
            max_instances_state: 32,
            cache_byte_budget: 64 * 1024 * 1024,
        }
    }
}

/// Erros tipados do WorkflowSpace (REQ-900-014: causa explícita, sem
/// degradação silenciosa).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum WorkflowSpaceError {
    #[error("capacidade cheia no tópico {topic}: {have}/{limit} instâncias")]
    CapacityFull {
        topic: &'static str,
        have: usize,
        limit: usize,
    },
    #[error("schema_version inválido no tópico {topic}: esperado {expected}, recebido {got}")]
    InvalidSchemaVersion {
        topic: &'static str,
        expected: u16,
        got: u16,
    },
    #[error("amostra atrasada no tópico {topic} chave {key}: revisão recebida {received} <= corrente {current}")]
    Stale {
        topic: &'static str,
        key: String,
        current: u64,
        received: u64,
    },
    #[error("conflito no tópico {topic} chave {key}: mesma revisão {revision} com payload_hash diferente")]
    Conflict {
        topic: &'static str,
        key: String,
        revision: u64,
    },
    #[error("orçamento de cache excedido ({used}/{budget} bytes) no tópico {topic}")]
    CacheBudgetExceeded {
        topic: &'static str,
        used: usize,
        budget: usize,
    },
    #[error("DDS error: {0}")]
    Dds(String),
    #[error("write failed: {0}")]
    WriteFailed(String),
}

impl WorkflowSpaceError {
    /// Tópico envolvido (para telemetria/admissão).
    pub fn topic(&self) -> Option<&'static str> {
        match self {
            Self::CapacityFull { topic, .. }
            | Self::InvalidSchemaVersion { topic, .. }
            | Self::Stale { topic, .. }
            | Self::Conflict { topic, .. }
            | Self::CacheBudgetExceeded { topic, .. } => Some(topic),
            _ => None,
        }
    }
}

type SharedStream<T> = Pin<Box<dyn Stream<Item = Arc<T>> + Send>>;

/// Interface dos 5 tópicos de roteamento adaptativo (composição; M1-03).
#[async_trait::async_trait]
pub trait WorkflowSpaceApi: Send + Sync {
    async fn write_intent(&self, intent: WorkflowIntent) -> Result<(), WorkflowSpaceError>;
    async fn read_intent(
        &self,
        request_id: &str,
    ) -> Result<Option<Arc<WorkflowIntent>>, WorkflowSpaceError>;
    fn subscribe_intents(&self) -> SharedStream<WorkflowIntent>;

    async fn write_decision(&self, decision: RoutingDecision) -> Result<(), WorkflowSpaceError>;
    async fn read_decision(
        &self,
        request_id: &str,
        decision_id: &str,
    ) -> Result<Option<Arc<RoutingDecision>>, WorkflowSpaceError>;
    fn subscribe_decisions(&self) -> SharedStream<RoutingDecision>;

    async fn write_capabilities(&self, caps: AgentCapabilities) -> Result<(), WorkflowSpaceError>;
    async fn read_capabilities(
        &self,
        agent_id: &str,
    ) -> Result<Option<Arc<AgentCapabilities>>, WorkflowSpaceError>;
    fn subscribe_capabilities(&self) -> SharedStream<AgentCapabilities>;

    async fn write_plan(&self, plan: WorkflowPlan) -> Result<(), WorkflowSpaceError>;
    async fn read_plan(
        &self,
        workflow_id: &str,
        plan_revision: u32,
    ) -> Result<Option<Arc<WorkflowPlan>>, WorkflowSpaceError>;
    fn subscribe_plans(&self) -> SharedStream<WorkflowPlan>;

    async fn write_state(&self, state: WorkflowState) -> Result<(), WorkflowSpaceError>;
    async fn read_state(
        &self,
        workflow_id: &str,
        plan_revision: u32,
    ) -> Result<Option<Arc<WorkflowState>>, WorkflowSpaceError>;
    fn subscribe_states(&self) -> SharedStream<WorkflowState>;

    /// Contagem de instâncias por tópico (observabilidade/limites).
    fn instance_counts(&self) -> HashMap<&'static str, usize>;

    async fn shutdown(&self) -> Result<(), WorkflowSpaceError>;
}

/// Estimativa de bytes de uma amostra no cache de aplicação (aproximação:
/// tamanho fixo do struct + strings; usada apenas pelo orçamento de cache).
pub mod bytes_estimate {
    use dds_contract::workflow::dds_llm_workflow_v1 as w;

    /// Custos por amostra: fixo do struct + Σ strings.
    pub fn intent(s: &w::WorkflowIntent) -> usize {
        std::mem::size_of::<w::WorkflowIntent>()
            + s.request_id.to_string().len()
            + s.workflow_id.to_string().len()
            + s.client_id.to_string().len()
            + s.prompt.to_string().len()
            + s.security_context_ref.to_string().len()
            + s.budget_unit.to_string().len()
            + s.checkpoint_revision.to_string().len()
            + s.fallback_template_id.to_string().len()
            + s.payload_hash.to_string().len()
    }

    pub fn decision(s: &w::RoutingDecision) -> usize {
        std::mem::size_of::<w::RoutingDecision>()
            + s.request_id.to_string().len()
            + s.decision_id.to_string().len()
            + s.workflow_id.to_string().len()
            + s.producer_id.to_string().len()
            + s.model_id.to_string().len()
            + s.checkpoint_revision.to_string().len()
            + s.calibration_revision.to_string().len()
            + s.evidence_digest.to_string().len()
            + s.payload_hash.to_string().len()
            + s.qos_probabilities.len() * std::mem::size_of::<w::ProfileProbability>()
            + s.capability_probabilities.len() * std::mem::size_of::<w::CapabilityProbability>()
    }

    pub fn capabilities(s: &w::AgentCapabilities) -> usize {
        std::mem::size_of::<w::AgentCapabilities>()
            + s.agent_id.to_string().len()
            + s.security_context_ref.to_string().len()
            + s.capabilities
                .iter()
                .map(|c| c.to_string().len() + 8)
                .sum::<usize>()
            + s.model_ids
                .iter()
                .map(|c| c.to_string().len() + 8)
                .sum::<usize>()
            + s.tool_versions.len() * std::mem::size_of::<w::ToolVersion>()
    }

    pub fn plan(s: &w::WorkflowPlan) -> usize {
        std::mem::size_of::<w::WorkflowPlan>()
            + s.workflow_id.to_string().len()
            + s.request_id.to_string().len()
            + s.effective_decision_id.to_string().len()
            + s.planner_id.to_string().len()
            + s.planner_revision.to_string().len()
            + s.plan_hash.to_string().len()
            + s.nodes.len() * 256 // estimativa conservadora por nó (prompt_template <= 8 KiB)
    }

    pub fn state(s: &w::WorkflowState) -> usize {
        std::mem::size_of::<w::WorkflowState>()
            + s.workflow_id.to_string().len()
            + s.request_id.to_string().len()
            + s.effective_decision_id.to_string().len()
            + s.plan_hash.to_string().len()
            + s.cancelled_by_request_id.to_string().len()
            + s.effective_qos_config_hash.to_string().len()
            + s.fallback_template_id.to_string().len()
            + s.nodes.len() * 128
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_defaults_match_contract_table() {
        let l = WorkflowSpaceLimits::default();
        assert_eq!(l.max_instances_intent, 64);
        assert_eq!(l.max_instances_decision, 256);
        assert_eq!(l.max_instances_capabilities, 256);
        assert_eq!(l.max_instances_plan, 32);
        assert_eq!(l.max_instances_state, 32);
        assert_eq!(l.cache_byte_budget, 64 * 1024 * 1024);
    }
}

// ============================================================================
// Caches + admissão compartilhados entre backends
// ============================================================================

use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;

const BROADCAST_CAPACITY: usize = 256;

/// Estado interno validado (KeepLast(1) por chave) + broadcast de amostras
/// ACEITAS (rejeitadas não são entregues — espelha o contrato do cache real).
pub(crate) struct WorkflowCaches {
    pub(crate) limits: WorkflowSpaceLimits,
    intents: DashMap<String, Arc<WorkflowIntent>>,
    decisions: DashMap<String, Arc<RoutingDecision>>,
    capabilities: DashMap<String, Arc<AgentCapabilities>>,
    plans: DashMap<String, Arc<WorkflowPlan>>,
    states: DashMap<String, Arc<WorkflowState>>,
    intent_tx: broadcast::Sender<Arc<WorkflowIntent>>,
    decision_tx: broadcast::Sender<Arc<RoutingDecision>>,
    capability_tx: broadcast::Sender<Arc<AgentCapabilities>>,
    plan_tx: broadcast::Sender<Arc<WorkflowPlan>>,
    state_tx: broadcast::Sender<Arc<WorkflowState>>,
    bytes_used: AtomicUsize,
}

impl WorkflowCaches {
    pub(crate) fn new(limits: WorkflowSpaceLimits) -> Self {
        let (intent_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (decision_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (capability_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (plan_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (state_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            limits,
            intents: DashMap::new(),
            decisions: DashMap::new(),
            capabilities: DashMap::new(),
            plans: DashMap::new(),
            states: DashMap::new(),
            intent_tx,
            decision_tx,
            capability_tx,
            plan_tx,
            state_tx,
            bytes_used: AtomicUsize::new(0),
        }
    }

    pub(crate) fn instance_counts(&self) -> HashMap<&'static str, usize> {
        HashMap::from([
            ("Workflow.Intent", self.intents.len()),
            ("Routing.Decision", self.decisions.len()),
            ("Agent.Capabilities", self.capabilities.len()),
            ("Workflow.Plan", self.plans.len()),
            ("Workflow.State", self.states.len()),
        ])
    }

    /// Bytes de cache de aplicação em uso (orçamento de 64 MiB).
    pub(crate) fn bytes_used(&self) -> usize {
        self.bytes_used.load(Ordering::Relaxed)
    }

    /// Capacidade + orçamento para uma chave NOVA; substituição de chave
    /// existente devolve os bytes da amostra antiga.
    fn admit_capacity(
        &self,
        topic: &'static str,
        is_new_key: bool,
        have: usize,
        limit: usize,
        old_bytes: usize,
        new_bytes: usize,
    ) -> Result<(), WorkflowSpaceError> {
        if is_new_key && have >= limit {
            return Err(WorkflowSpaceError::CapacityFull { topic, have, limit });
        }
        let used = self.bytes_used.load(Ordering::Relaxed);
        let projected = used + new_bytes.saturating_sub(old_bytes);
        if projected > self.limits.cache_byte_budget {
            return Err(WorkflowSpaceError::CacheBudgetExceeded {
                topic,
                used,
                budget: self.limits.cache_byte_budget,
            });
        }
        self.bytes_used.store(projected, Ordering::Relaxed);
        Ok(())
    }

    // ---- Intent ----------------------------------------------------------

    pub(crate) fn admit_intent(
        &self,
        s: WorkflowIntent,
    ) -> Result<Arc<WorkflowIntent>, WorkflowSpaceError> {
        const TOPIC: &str = "Workflow.Intent";
        if s.schema_version != SCHEMA_VERSION_V1 {
            return Err(WorkflowSpaceError::InvalidSchemaVersion {
                topic: TOPIC,
                expected: SCHEMA_VERSION_V1,
                got: s.schema_version,
            });
        }
        let key = s.request_id.to_string();
        let bytes = bytes_estimate::intent(&s);
        let old_bytes = match self.intents.get(&key) {
            Some(cur) => {
                let cur: &WorkflowIntent = &cur;
                if s.intent_revision < cur.intent_revision {
                    return Err(WorkflowSpaceError::Stale {
                        topic: TOPIC,
                        key,
                        current: cur.intent_revision as u64,
                        received: s.intent_revision as u64,
                    });
                }
                if s.intent_revision == cur.intent_revision && s.payload_hash != cur.payload_hash {
                    return Err(WorkflowSpaceError::Conflict {
                        topic: TOPIC,
                        key,
                        revision: s.intent_revision as u64,
                    });
                }
                bytes_estimate::intent(cur)
            }
            None => 0,
        };
        let is_new = !self.intents.contains_key(&key);
        self.admit_capacity(
            TOPIC,
            is_new,
            self.intents.len(),
            self.limits.max_instances_intent,
            old_bytes,
            bytes,
        )?;
        let arc = Arc::new(s);
        self.intents.insert(key, Arc::clone(&arc));
        let _ = self.intent_tx.send(arc.clone());
        Ok(arc)
    }

    pub(crate) fn read_intent(&self, request_id: &str) -> Option<Arc<WorkflowIntent>> {
        self.intents.get(request_id).map(|r| r.clone())
    }

    // ---- Decision --------------------------------------------------------

    pub(crate) fn admit_decision(
        &self,
        s: RoutingDecision,
    ) -> Result<Arc<RoutingDecision>, WorkflowSpaceError> {
        const TOPIC: &str = "Routing.Decision";
        if s.schema_version != SCHEMA_VERSION_V1 {
            return Err(WorkflowSpaceError::InvalidSchemaVersion {
                topic: TOPIC,
                expected: SCHEMA_VERSION_V1,
                got: s.schema_version,
            });
        }
        let key = format!("{}|{}", s.request_id, s.decision_id);
        let bytes = bytes_estimate::decision(&s);
        if let Some(cur) = self.decisions.get(&key) {
            let cur: &RoutingDecision = &cur;
            // Imutável por (request_id, decision_id): só aceita replay idêntico.
            if s.payload_hash != cur.payload_hash {
                return Err(WorkflowSpaceError::Conflict {
                    topic: TOPIC,
                    key,
                    revision: 0,
                });
            }
            let arc = Arc::new(s);
            let _ = self.decision_tx.send(arc.clone());
            return Ok(arc);
        }
        self.admit_capacity(
            TOPIC,
            true,
            self.decisions.len(),
            self.limits.max_instances_decision,
            0,
            bytes,
        )?;
        let arc = Arc::new(s);
        self.decisions.insert(key, Arc::clone(&arc));
        let _ = self.decision_tx.send(arc.clone());
        Ok(arc)
    }

    pub(crate) fn read_decision(
        &self,
        request_id: &str,
        decision_id: &str,
    ) -> Option<Arc<RoutingDecision>> {
        self.decisions
            .get(&format!("{request_id}|{decision_id}"))
            .map(|r| r.clone())
    }

    // ---- Capabilities ----------------------------------------------------

    pub(crate) fn admit_capabilities(
        &self,
        s: AgentCapabilities,
    ) -> Result<Arc<AgentCapabilities>, WorkflowSpaceError> {
        const TOPIC: &str = "Agent.Capabilities";
        if s.schema_version != SCHEMA_VERSION_V1 {
            return Err(WorkflowSpaceError::InvalidSchemaVersion {
                topic: TOPIC,
                expected: SCHEMA_VERSION_V1,
                got: s.schema_version,
            });
        }
        let key = s.agent_id.to_string();
        let bytes = bytes_estimate::capabilities(&s);
        let old_bytes = match self.capabilities.get(&key) {
            Some(cur) => {
                let cur: &AgentCapabilities = &cur;
                if s.announcement_revision < cur.announcement_revision {
                    return Err(WorkflowSpaceError::Stale {
                        topic: TOPIC,
                        key,
                        current: cur.announcement_revision as u64,
                        received: s.announcement_revision as u64,
                    });
                }
                bytes_estimate::capabilities(cur)
            }
            None => 0,
        };
        let is_new = !self.capabilities.contains_key(&key);
        self.admit_capacity(
            TOPIC,
            is_new,
            self.capabilities.len(),
            self.limits.max_instances_capabilities,
            old_bytes,
            bytes,
        )?;
        let arc = Arc::new(s);
        self.capabilities.insert(key, Arc::clone(&arc));
        let _ = self.capability_tx.send(arc.clone());
        Ok(arc)
    }

    pub(crate) fn read_capabilities(&self, agent_id: &str) -> Option<Arc<AgentCapabilities>> {
        self.capabilities.get(agent_id).map(|r| r.clone())
    }

    // ---- Plan ------------------------------------------------------------

    pub(crate) fn admit_plan(
        &self,
        s: WorkflowPlan,
    ) -> Result<Arc<WorkflowPlan>, WorkflowSpaceError> {
        const TOPIC: &str = "Workflow.Plan";
        if s.schema_version != SCHEMA_VERSION_V1 {
            return Err(WorkflowSpaceError::InvalidSchemaVersion {
                topic: TOPIC,
                expected: SCHEMA_VERSION_V1,
                got: s.schema_version,
            });
        }
        let key = format!("{}|{}", s.workflow_id, s.plan_revision);
        let bytes = bytes_estimate::plan(&s);
        if let Some(cur) = self.plans.get(&key) {
            let cur: &WorkflowPlan = &cur;
            if s.plan_hash != cur.plan_hash {
                return Err(WorkflowSpaceError::Conflict {
                    topic: TOPIC,
                    key,
                    revision: s.plan_revision as u64,
                });
            }
            let arc = Arc::new(s);
            let _ = self.plan_tx.send(arc.clone());
            return Ok(arc);
        }
        self.admit_capacity(
            TOPIC,
            true,
            self.plans.len(),
            self.limits.max_instances_plan,
            0,
            bytes,
        )?;
        let arc = Arc::new(s);
        self.plans.insert(key, Arc::clone(&arc));
        let _ = self.plan_tx.send(arc.clone());
        Ok(arc)
    }

    pub(crate) fn read_plan(
        &self,
        workflow_id: &str,
        plan_revision: u32,
    ) -> Option<Arc<WorkflowPlan>> {
        self.plans
            .get(&format!("{workflow_id}|{plan_revision}"))
            .map(|r| r.clone())
    }

    // ---- State -----------------------------------------------------------

    pub(crate) fn admit_state(
        &self,
        s: WorkflowState,
    ) -> Result<Arc<WorkflowState>, WorkflowSpaceError> {
        const TOPIC: &str = "Workflow.State";
        if s.schema_version != SCHEMA_VERSION_V1 {
            return Err(WorkflowSpaceError::InvalidSchemaVersion {
                topic: TOPIC,
                expected: SCHEMA_VERSION_V1,
                got: s.schema_version,
            });
        }
        let key = format!("{}|{}", s.workflow_id, s.plan_revision);
        let bytes = bytes_estimate::state(&s);
        let old_bytes = match self.states.get(&key) {
            Some(cur) => {
                let cur: &WorkflowState = &cur;
                if s.state_revision < cur.state_revision {
                    return Err(WorkflowSpaceError::Stale {
                        topic: TOPIC,
                        key,
                        current: cur.state_revision as u64,
                        received: s.state_revision as u64,
                    });
                }
                if s.state_revision == cur.state_revision && s != *cur {
                    // Mesma revisão, conteúdo diferente = conflito (contracts).
                    return Err(WorkflowSpaceError::Conflict {
                        topic: TOPIC,
                        key,
                        revision: s.state_revision as u64,
                    });
                }
                bytes_estimate::state(cur)
            }
            None => 0,
        };
        let is_new = !self.states.contains_key(&key);
        self.admit_capacity(
            TOPIC,
            is_new,
            self.states.len(),
            self.limits.max_instances_state,
            old_bytes,
            bytes,
        )?;
        let arc = Arc::new(s);
        self.states.insert(key, Arc::clone(&arc));
        let _ = self.state_tx.send(arc.clone());
        Ok(arc)
    }

    pub(crate) fn read_state(
        &self,
        workflow_id: &str,
        plan_revision: u32,
    ) -> Option<Arc<WorkflowState>> {
        self.states
            .get(&format!("{workflow_id}|{plan_revision}"))
            .map(|r| r.clone())
    }
}

/// Macro espelhando `impl_subscribe!` do `in_memory.rs`.
macro_rules! impl_workflow_subscribe {
    ($fn_name:ident, $rx_field:ident, $item:ty) => {
        fn $fn_name(&self) -> Pin<Box<dyn Stream<Item = Arc<$item>> + Send>> {
            let mut rx = self.caches.$rx_field.subscribe();
            Box::pin(async_stream::stream! {
                loop {
                    match rx.recv().await {
                        Ok(item) => yield item,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            })
        }
    };
}

// ============================================================================
// InMemoryWorkflowSpace
// ============================================================================

/// Backend de teste/ablação (EXP1a) dos 5 tópicos da fase 900: validação de
/// contrato idêntica ao caminho DDS (mesma `WorkflowCaches::admit_*`) — erro
/// TIPADO em capacidade/versão/lag/conflito, nunca descarte silencioso
/// (divergência deliberada vs `InMemoryDataSpace::write_task`, que descarta
/// silenciosamente por paridade com o cache legado).
pub struct InMemoryWorkflowSpace {
    caches: Arc<WorkflowCaches>,
}

impl Default for InMemoryWorkflowSpace {
    fn default() -> Self {
        Self::new(WorkflowSpaceLimits::default())
    }
}

impl InMemoryWorkflowSpace {
    pub fn new(limits: WorkflowSpaceLimits) -> Self {
        Self {
            caches: Arc::new(WorkflowCaches::new(limits)),
        }
    }

    /// Bytes de cache de aplicação em uso (orçamento de 64 MiB).
    pub fn bytes_used(&self) -> usize {
        self.caches.bytes_used()
    }

    /// Snapshot completo para replay/inspeção (aceite M1-03: snapshots/replay
    /// não perdem estado silenciosamente — os counts conferem com
    /// `instance_counts`).
    pub fn snapshot(&self) -> WorkflowSnapshot {
        WorkflowSnapshot {
            intents: self.caches.intents.iter().map(|r| r.clone()).collect(),
            decisions: self.caches.decisions.iter().map(|r| r.clone()).collect(),
            capabilities: self.caches.capabilities.iter().map(|r| r.clone()).collect(),
            plans: self.caches.plans.iter().map(|r| r.clone()).collect(),
            states: self.caches.states.iter().map(|r| r.clone()).collect(),
        }
    }

    /// Restaura um snapshot NUM ESPAÇO VAZIO (replay pós-restart). Amostras
    /// rejeitadas (stale/conflito) retornam erro tipado SEM alterar o estado
    /// já restaurado — sem perda silenciosa.
    pub fn restore(&self, snap: WorkflowSnapshot) -> Result<(), WorkflowSpaceError> {
        for i in snap.intents {
            self.caches.admit_intent((*i).clone())?;
        }
        for d in snap.decisions {
            self.caches.admit_decision((*d).clone())?;
        }
        for c in snap.capabilities {
            self.caches.admit_capabilities((*c).clone())?;
        }
        for p in snap.plans {
            self.caches.admit_plan((*p).clone())?;
        }
        for s in snap.states {
            self.caches.admit_state((*s).clone())?;
        }
        Ok(())
    }
}

/// Foto do estado interno dos 5 tópicos (chaves + amostras aceitas).
#[derive(Debug, Clone, Default)]
pub struct WorkflowSnapshot {
    pub intents: Vec<Arc<WorkflowIntent>>,
    pub decisions: Vec<Arc<RoutingDecision>>,
    pub capabilities: Vec<Arc<AgentCapabilities>>,
    pub plans: Vec<Arc<WorkflowPlan>>,
    pub states: Vec<Arc<WorkflowState>>,
}

#[async_trait::async_trait]
impl WorkflowSpaceApi for InMemoryWorkflowSpace {
    async fn write_intent(&self, intent: WorkflowIntent) -> Result<(), WorkflowSpaceError> {
        self.caches.admit_intent(intent).map(|_| ())
    }
    async fn read_intent(
        &self,
        request_id: &str,
    ) -> Result<Option<Arc<WorkflowIntent>>, WorkflowSpaceError> {
        Ok(self.caches.read_intent(request_id))
    }
    impl_workflow_subscribe!(subscribe_intents, intent_tx, WorkflowIntent);

    async fn write_decision(&self, decision: RoutingDecision) -> Result<(), WorkflowSpaceError> {
        self.caches.admit_decision(decision).map(|_| ())
    }
    async fn read_decision(
        &self,
        request_id: &str,
        decision_id: &str,
    ) -> Result<Option<Arc<RoutingDecision>>, WorkflowSpaceError> {
        Ok(self.caches.read_decision(request_id, decision_id))
    }
    impl_workflow_subscribe!(subscribe_decisions, decision_tx, RoutingDecision);

    async fn write_capabilities(&self, caps: AgentCapabilities) -> Result<(), WorkflowSpaceError> {
        self.caches.admit_capabilities(caps).map(|_| ())
    }
    async fn read_capabilities(
        &self,
        agent_id: &str,
    ) -> Result<Option<Arc<AgentCapabilities>>, WorkflowSpaceError> {
        Ok(self.caches.read_capabilities(agent_id))
    }
    impl_workflow_subscribe!(subscribe_capabilities, capability_tx, AgentCapabilities);

    async fn write_plan(&self, plan: WorkflowPlan) -> Result<(), WorkflowSpaceError> {
        self.caches.admit_plan(plan).map(|_| ())
    }
    async fn read_plan(
        &self,
        workflow_id: &str,
        plan_revision: u32,
    ) -> Result<Option<Arc<WorkflowPlan>>, WorkflowSpaceError> {
        Ok(self.caches.read_plan(workflow_id, plan_revision))
    }
    impl_workflow_subscribe!(subscribe_plans, plan_tx, WorkflowPlan);

    async fn write_state(&self, state: WorkflowState) -> Result<(), WorkflowSpaceError> {
        self.caches.admit_state(state).map(|_| ())
    }
    async fn read_state(
        &self,
        workflow_id: &str,
        plan_revision: u32,
    ) -> Result<Option<Arc<WorkflowState>>, WorkflowSpaceError> {
        Ok(self.caches.read_state(workflow_id, plan_revision))
    }
    impl_workflow_subscribe!(subscribe_states, state_tx, WorkflowState);

    fn instance_counts(&self) -> HashMap<&'static str, usize> {
        self.caches.instance_counts()
    }

    async fn shutdown(&self) -> Result<(), WorkflowSpaceError> {
        Ok(())
    }
}

// ============================================================================
// DdsWorkflowSpace (feature `dds`)
// ============================================================================

#[cfg(feature = "dds")]
pub mod dds_impl {
    //! Backend DDS real dos 5 tópicos da fase 900 (T-900-M1-03).
    //!
    //! Entidades com os perfis `qos::profiles::workflow` (contracts.md §"QoS
    //! por entidade"). Divergência documentada vs `DataSpace` (T-617): as
    //! `subscribe_*` usam `take_aiter` (um DataReader por stream, padrão
    //! pré-waitset) — os consumidores da fase 900 são poucos (engine único +
    //! worker + agente opt-in); migração para `SharedWaitSet` fica para M5 se
    //! a medição pedir. Escrita valida ANTES de publicar (fail-closed:
    //! amostra inválida não circula na malha). Leitura serve do cache local
    //! (alimentado pelas streams e pelas próprias escritas) com fallback
    //! `read()` de um reader dedicado (TransientLocal + KeepLast(1)).

    use super::{
        AgentCapabilities, RoutingDecision, WorkflowCaches, WorkflowIntent, WorkflowPlan,
        WorkflowSpaceApi, WorkflowSpaceError, WorkflowSpaceLimits, WorkflowState,
    };
    use crate::qos;
    use cyclonedds::{DataReader, DataWriter, DomainParticipant, Publisher, Subscriber, Topic};
    use dds_contract::workflow_topics;
    use futures::StreamExt;
    use futures_core::Stream;
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::sync::Arc;

    pub struct DdsWorkflowSpace {
        // Mantém o participant vivo enquanto writers/readers existem.
        _participant: DomainParticipant,
        subscriber: Subscriber,
        intent_topic: Arc<Topic<WorkflowIntent>>,
        intent_writer: DataWriter<WorkflowIntent>,
        decision_topic: Arc<Topic<RoutingDecision>>,
        decision_writer: DataWriter<RoutingDecision>,
        caps_topic: Arc<Topic<AgentCapabilities>>,
        caps_writer: DataWriter<AgentCapabilities>,
        plan_topic: Arc<Topic<WorkflowPlan>>,
        plan_writer: DataWriter<WorkflowPlan>,
        state_topic: Arc<Topic<WorkflowState>>,
        state_writer: DataWriter<WorkflowState>,
        pub(crate) caches: Arc<WorkflowCaches>,
    }

    fn err(e: cyclonedds::DdsError) -> WorkflowSpaceError {
        WorkflowSpaceError::Dds(e.to_string())
    }

    /// Stream genérica de um tópico: reader dedicado (criado ansiosamente) +
    /// `take_aiter`; amostra passa pela mesma admissão do cache (rejeitada
    /// não é entregue) — espelha o contrato do `DataSpace` (stream só entrega
    /// o que o cache aceitou).
    fn topic_stream<T, F>(
        subscriber: &Subscriber,
        topic: &Topic<T>,
        admit: F,
    ) -> Pin<Box<dyn Stream<Item = Arc<T>> + Send>>
    where
        T: cyclonedds::DdsType + Send + Sync + 'static,
        F: Fn(T) -> Result<Arc<T>, WorkflowSpaceError> + Send + 'static,
    {
        let reader = match DataReader::new(subscriber, topic) {
            Ok(r) => r,
            Err(_) => {
                return Box::pin(futures::stream::empty());
            }
        };
        Box::pin(async_stream::stream! {
            let mut it = Box::pin(reader.take_aiter());
            while let Some(batch) = it.next().await {
                match batch {
                    Ok(samples) => {
                        for s in samples {
                            if let Ok(arc) = admit(s) {
                                yield arc;
                            }
                        }
                    }
                    Err(_) => {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        })
    }

    impl DdsWorkflowSpace {
        pub fn new(
            domain_id: u32,
            limits: WorkflowSpaceLimits,
        ) -> Result<Self, WorkflowSpaceError> {
            let participant = DomainParticipant::new(domain_id).map_err(err)?;
            let publisher = Publisher::new(&participant).map_err(err)?;
            let subscriber = Subscriber::new(&participant).map_err(err)?;

            macro_rules! endpoint {
                ($ty:ty, $const:expr, $profile:expr) => {{
                    let q = $profile.map_err(err)?;
                    let topic =
                        Topic::<$ty>::with_qos(&participant, $const, Some(&q)).map_err(err)?;
                    let writer = DataWriter::with_qos(&publisher, &topic, Some(&q)).map_err(err)?;
                    (Arc::new(topic), writer)
                }};
            }

            let (intent_topic, intent_writer) = endpoint!(
                WorkflowIntent,
                workflow_topics::WORKFLOW_INTENT,
                qos::profiles::workflow::intent()
            );
            let (decision_topic, decision_writer) = endpoint!(
                RoutingDecision,
                workflow_topics::ROUTING_DECISION,
                qos::profiles::workflow::decision()
            );
            let (caps_topic, caps_writer) = endpoint!(
                AgentCapabilities,
                workflow_topics::AGENT_CAPABILITIES,
                qos::profiles::workflow::capabilities()
            );
            let (plan_topic, plan_writer) = endpoint!(
                WorkflowPlan,
                workflow_topics::WORKFLOW_PLAN,
                qos::profiles::workflow::plan()
            );
            let (state_topic, state_writer) = endpoint!(
                WorkflowState,
                workflow_topics::WORKFLOW_STATE,
                qos::profiles::workflow::state()
            );

            Ok(Self {
                _participant: participant,
                subscriber,
                intent_topic,
                intent_writer,
                decision_topic,
                decision_writer,
                caps_topic,
                caps_writer,
                plan_topic,
                plan_writer,
                state_topic,
                state_writer,
                caches: Arc::new(WorkflowCaches::new(limits)),
            })
        }

        /// `read()` de um reader dedicado (não drena o RHC; KeepLast(1) por
        /// chave limita o histórico retornado).
        fn read_latest<T: cyclonedds::DdsType>(
            &self,
            topic: &Topic<T>,
        ) -> Result<Option<T>, WorkflowSpaceError> {
            let reader = DataReader::new(&self.subscriber, topic).map_err(err)?;
            let mut samples = reader.read().map_err(err)?;
            Ok(samples.pop())
        }
    }

    #[async_trait::async_trait]
    impl WorkflowSpaceApi for DdsWorkflowSpace {
        async fn write_intent(&self, intent: WorkflowIntent) -> Result<(), WorkflowSpaceError> {
            self.caches.admit_intent(intent.clone())?;
            self.intent_writer
                .write(&intent)
                .map_err(|e| WorkflowSpaceError::WriteFailed(e.to_string()))
        }

        async fn read_intent(
            &self,
            request_id: &str,
        ) -> Result<Option<Arc<WorkflowIntent>>, WorkflowSpaceError> {
            if let Some(hit) = self.caches.read_intent(request_id) {
                return Ok(Some(hit));
            }
            Ok(self
                .read_latest(&self.intent_topic)?
                .filter(|i| i.request_id.to_string() == request_id)
                .map(Arc::new))
        }

        fn subscribe_intents(&self) -> Pin<Box<dyn Stream<Item = Arc<WorkflowIntent>> + Send>> {
            let caches = self.caches.clone();
            topic_stream(&self.subscriber, &self.intent_topic, move |s| {
                caches.admit_intent(s)
            })
        }

        async fn write_decision(
            &self,
            decision: RoutingDecision,
        ) -> Result<(), WorkflowSpaceError> {
            self.caches.admit_decision(decision.clone())?;
            self.decision_writer
                .write(&decision)
                .map_err(|e| WorkflowSpaceError::WriteFailed(e.to_string()))
        }

        async fn read_decision(
            &self,
            request_id: &str,
            decision_id: &str,
        ) -> Result<Option<Arc<RoutingDecision>>, WorkflowSpaceError> {
            if let Some(hit) = self.caches.read_decision(request_id, decision_id) {
                return Ok(Some(hit));
            }
            self.read_latest(&self.decision_topic).map(|o| {
                o.filter(|d| {
                    d.request_id.to_string() == request_id
                        && d.decision_id.to_string() == decision_id
                })
                .map(Arc::new)
            })
        }

        fn subscribe_decisions(&self) -> Pin<Box<dyn Stream<Item = Arc<RoutingDecision>> + Send>> {
            let caches = self.caches.clone();
            topic_stream(&self.subscriber, &self.decision_topic, move |s| {
                caches.admit_decision(s)
            })
        }

        async fn write_capabilities(
            &self,
            caps: AgentCapabilities,
        ) -> Result<(), WorkflowSpaceError> {
            self.caches.admit_capabilities(caps.clone())?;
            self.caps_writer
                .write(&caps)
                .map_err(|e| WorkflowSpaceError::WriteFailed(e.to_string()))
        }

        async fn read_capabilities(
            &self,
            agent_id: &str,
        ) -> Result<Option<Arc<AgentCapabilities>>, WorkflowSpaceError> {
            if let Some(hit) = self.caches.read_capabilities(agent_id) {
                return Ok(Some(hit));
            }
            self.read_latest(&self.caps_topic).map(|o| {
                o.filter(|c| c.agent_id.to_string() == agent_id)
                    .map(Arc::new)
            })
        }

        fn subscribe_capabilities(
            &self,
        ) -> Pin<Box<dyn Stream<Item = Arc<AgentCapabilities>> + Send>> {
            let caches = self.caches.clone();
            topic_stream(&self.subscriber, &self.caps_topic, move |s| {
                caches.admit_capabilities(s)
            })
        }

        async fn write_plan(&self, plan: WorkflowPlan) -> Result<(), WorkflowSpaceError> {
            self.caches.admit_plan(plan.clone())?;
            self.plan_writer
                .write(&plan)
                .map_err(|e| WorkflowSpaceError::WriteFailed(e.to_string()))
        }

        async fn read_plan(
            &self,
            workflow_id: &str,
            plan_revision: u32,
        ) -> Result<Option<Arc<WorkflowPlan>>, WorkflowSpaceError> {
            if let Some(hit) = self.caches.read_plan(workflow_id, plan_revision) {
                return Ok(Some(hit));
            }
            self.read_latest(&self.plan_topic).map(|o| {
                o.filter(|p| {
                    p.workflow_id.to_string() == workflow_id && p.plan_revision == plan_revision
                })
                .map(Arc::new)
            })
        }

        fn subscribe_plans(&self) -> Pin<Box<dyn Stream<Item = Arc<WorkflowPlan>> + Send>> {
            let caches = self.caches.clone();
            topic_stream(&self.subscriber, &self.plan_topic, move |s| {
                caches.admit_plan(s)
            })
        }

        async fn write_state(&self, state: WorkflowState) -> Result<(), WorkflowSpaceError> {
            self.caches.admit_state(state.clone())?;
            self.state_writer
                .write(&state)
                .map_err(|e| WorkflowSpaceError::WriteFailed(e.to_string()))
        }

        async fn read_state(
            &self,
            workflow_id: &str,
            plan_revision: u32,
        ) -> Result<Option<Arc<WorkflowState>>, WorkflowSpaceError> {
            if let Some(hit) = self.caches.read_state(workflow_id, plan_revision) {
                return Ok(Some(hit));
            }
            self.read_latest(&self.state_topic).map(|o| {
                o.filter(|s| {
                    s.workflow_id.to_string() == workflow_id && s.plan_revision == plan_revision
                })
                .map(Arc::new)
            })
        }

        fn subscribe_states(&self) -> Pin<Box<dyn Stream<Item = Arc<WorkflowState>> + Send>> {
            let caches = self.caches.clone();
            topic_stream(&self.subscriber, &self.state_topic, move |s| {
                caches.admit_state(s)
            })
        }

        fn instance_counts(&self) -> HashMap<&'static str, usize> {
            self.caches.instance_counts()
        }

        async fn shutdown(&self) -> Result<(), WorkflowSpaceError> {
            Ok(())
        }
    }
}
