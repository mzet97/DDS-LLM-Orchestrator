//! # Orchestrator — Control Plane (Fase 3)
//!
//! Substitui `src/orchestrator/orchestrator/` (~2,0k LOC Python):
//! - API HTTP (axum) para submissão de tasks
//! - Scheduler com fila de prioridade
//! - Registry com monitoramento de liveliness
//! - Selector/Dispatcher por especialização
//! - Loop de controle com NFCM para decisão de QoS
//! - State machine para transições de task
//! - Failover cascading (T-424)

pub mod http;
pub mod http_config;
pub mod state_machine;

#[cfg(feature = "dds")]
pub mod dds;

#[cfg(feature = "dds")]
mod qos_routing;

#[cfg(feature = "dds")]
mod qos_monitor;

use dds_contract::generated::dds_llm_orchestrator::Task;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Prioridade de task para o scheduler.
#[derive(Debug, Clone)]
pub struct PrioritizedTask {
    pub task: Task,
    pub priority: i32,
    pub created_at_ns: u64,
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.created_at_ns == other.created_at_ns
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older tasks first
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.created_at_ns.cmp(&self.created_at_ns))
    }
}

/// Scheduler com fila de prioridade (REQ-402, T-402).
/// Capaz de no máximo `MAX_SCHEDULER_SIZE` tasks — as mais antigas são descartadas
/// quando o limite é atingido (DDS é a fonte de verdade, o scheduler é best-effort).
pub struct Scheduler {
    queue: BinaryHeap<PrioritizedTask>,
}

const MAX_SCHEDULER_SIZE: usize = 1024;

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }

    /// Enfileira uma task. Descarta a task de menor prioridade se a fila
    /// atingiu `MAX_SCHEDULER_SIZE` (best-effort — DDS é a fonte de verdade).
    pub fn push(&mut self, task: Task) {
        if self.queue.len() >= MAX_SCHEDULER_SIZE {
            // BinaryHeap é max-heap; para remover o menor, drena parcialmente.
            // Simples: descarta a task mais recente (menor prioridade temporal)
            // reconstruindo sem ela. Como é best-effort, apenas logamos.
            tracing::warn!(
                size = self.queue.len(),
                "scheduler: capacidade máxima atingida, descartando task mais antiga"
            );
            // Remove o item com menor prioridade (último no sort order).
            let mut items: Vec<_> = self.queue.drain().collect();
            items.sort();
            items.pop(); // remove lowest priority
            self.queue = items.into_iter().collect();
        }
        let prioritized = PrioritizedTask {
            priority: task.priority,
            created_at_ns: task.created_at_ns,
            task,
        };
        self.queue.push(prioritized);
    }

    /// Remove e retorna a task de maior prioridade.
    pub fn pop(&mut self) -> Option<Task> {
        self.queue.pop().map(|pt| pt.task)
    }

    /// Retorna o tamanho da fila.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Verifica se a fila está vazia.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Registry de agentes (REQ-403, T-403).
///
/// `ahash` em vez do hasher default (Fase 2 do `OPTIMIZATION_PLAN.md`) — lookup
/// de alta frequência no control loop (seleção de agente por task).
pub struct AgentRegistry {
    agents: dashmap::DashMap<
        String,
        dds_contract::generated::dds_llm_orchestrator::AgentState,
        ahash::RandomState,
    >,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: dashmap::DashMap::with_hasher(ahash::RandomState::default()),
        }
    }

    /// Registra ou atualiza um agente.
    pub fn upsert(&self, state: dds_contract::generated::dds_llm_orchestrator::AgentState) {
        self.agents.insert(state.agent_id.clone(), state);
    }

    /// Remove um agente.
    pub fn remove(&self, agent_id: &str) {
        self.agents.remove(agent_id);
    }

    /// Retorna um agente por ID.
    pub fn get(
        &self,
        agent_id: &str,
    ) -> Option<dds_contract::generated::dds_llm_orchestrator::AgentState> {
        self.agents.get(agent_id).map(|a| a.clone())
    }

    /// Lista todos os agentes.
    pub fn all(&self) -> Vec<dds_contract::generated::dds_llm_orchestrator::AgentState> {
        self.agents.iter().map(|a| a.clone()).collect()
    }

    /// Retorna agentes saudáveis com slots disponíveis.
    pub fn available(&self) -> Vec<dds_contract::generated::dds_llm_orchestrator::AgentState> {
        self.agents
            .iter()
            .filter(|a| a.health == 2 && a.slots_busy < a.slots_total) // HEALTHY
            .map(|a| a.clone())
            .collect()
    }
}

/// Selector — roteamento por especialização (REQ-404, T-404).
pub fn select_agent(
    task: &Task,
    agents: &[dds_contract::generated::dds_llm_orchestrator::AgentState],
) -> Option<dds_contract::generated::dds_llm_orchestrator::AgentState> {
    let eligible: Vec<_> = agents
        .iter()
        .filter(|a| {
            // HEALTHY
            a.health == 2
            // Has slots
            && a.slots_busy < a.slots_total
            // Specialization compatible
            && matches_specialization(a.specialization.as_str(), task.model_required)
            // Target agent compatible
            && (task.target_agent.is_empty() || a.agent_id.starts_with(&task.target_agent))
        })
        .collect();

    // Least-loaded (fewest busy slots)
    eligible.into_iter().min_by_key(|a| a.slots_busy).cloned()
}

fn matches_specialization(agent_spec: &str, required: i32) -> bool {
    match agent_spec.to_uppercase().as_str() {
        "TEXT" => true,                             // TEXT aceita qualquer coisa
        "VISION" => required == 0 || required == 1, // TEXT ou VISION
        "EMBEDDING" => required == 2,
        "TRANSCRIPTION" => required == 3,
        _ => false,
    }
}

/// Build failover chains from `FailoverConfig` and register them on `GatewayProviders`.
///
/// For each config, creates `FailoverTarget`s with circuit breakers and registers
/// them on the provider's primary name. The `provider_factory` closure maps a provider
/// name ("cloud", "local") to an `Arc<dyn LlmProvider>`.
///
/// Returns the updated `GatewayProviders` with failover targets registered.
pub fn build_failover_chains(
    providers: &mut llm_gateway::GatewayProviders,
    configs: &[llm_gateway::FailoverConfig],
    provider_factory: &dyn Fn(&str) -> Option<std::sync::Arc<dyn llm_gateway::LlmProvider>>,
) {
    use llm_gateway::FailoverTarget;

    for config in configs.iter() {
        let mut targets = Vec::new();
        for ft_config in config.targets.iter() {
            if let Some(provider) = provider_factory(&ft_config.provider) {
                targets.push(FailoverTarget {
                    provider,
                    model: ft_config.model.clone(),
                    circuit_breaker: std::sync::Arc::new(llm_gateway::CircuitBreaker::new(
                        ft_config.circuit_breaker.failure_threshold,
                        ft_config.circuit_breaker.recovery_timeout,
                    )),
                    priority: ft_config.priority,
                });
            }
        }
        if !targets.is_empty() {
            providers.register_failover(&config.primary_provider, targets);
        }
    }
}
