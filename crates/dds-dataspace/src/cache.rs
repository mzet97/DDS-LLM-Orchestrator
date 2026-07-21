//! Caches concorrentes por tópico (T-303, REQ-304/REQ-306).
//!
//! - `Arc<T>` imutável: leitores recebem snapshot consistente, sem lock de leitura.
//! - `DashMap` sharded: escrita de task não serializa com leitura de agente.
//! - **Regressão bloqueada por construção**: `upsert_task` só substitui se a nova
//!   versão *supera* a atual (status monotônico; empate pelo maior timestamp) —
//!   elimina as guardas anti-regressão (C1) do `dds_backend` Python.

use dashmap::DashMap;
use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, ContextSnapshot, ContextUpdate, DiscoveryEvent, ExecutionTraceEvent, QoSMetric,
    QoSRoutingProfile, QoSViolation, SecurityPolicySnapshot, SecurityPolicyUpdate, Task,
    TaskOutput, ToolCallRequest,
};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
};
use std::sync::Arc;

/// `DashMap` com `ahash` (Fase 2 do `OPTIMIZATION_PLAN.md`) em vez do hasher
/// default (`RandomState`/SipHash) — os caches de tópico são lookup/insert de
/// alta frequência no hot path (claim loop, writer pool, control loop).
pub type FastMap<K, V> = DashMap<K, V, ahash::RandomState>;

pub type ArcTask = Arc<Task>;
pub type ArcAgentState = Arc<AgentState>;
pub type ArcTaskOutput = Arc<TaskOutput>;
pub type ArcLLMRequest = Arc<LLMInferenceRequest>;
pub type ArcLLMResult = Arc<LLMInferenceResult>;
pub type ArcLLMError = Arc<LLMInferenceError>;
pub type ArcContextSnapshot = Arc<ContextSnapshot>;
pub type ArcContextUpdate = Arc<ContextUpdate>;
pub type ArcToolCallRequest = Arc<ToolCallRequest>;
pub type ArcExecutionTraceEvent = Arc<ExecutionTraceEvent>;
pub type ArcSecurityPolicySnapshot = Arc<SecurityPolicySnapshot>;
pub type ArcSecurityPolicyUpdate = Arc<SecurityPolicyUpdate>;
pub type ArcQoSRoutingProfile = Arc<QoSRoutingProfile>;
pub type ArcQoSMetric = Arc<QoSMetric>;
pub type ArcQoSViolation = Arc<QoSViolation>;
pub type ArcDiscoveryEvent = Arc<DiscoveryEvent>;

/// Caches do DataSpace (um por processo).
#[derive(Default)]
pub struct TopicCaches {
    // Tópicos originais (3)
    pub tasks: FastMap<String, ArcTask>,
    pub agents: FastMap<String, ArcAgentState>,
    pub outputs: FastMap<String, Vec<ArcTaskOutput>>,

    // Tópicos LLM (3)
    pub llm_requests: FastMap<String, ArcLLMRequest>,
    pub llm_results: FastMap<String, Vec<ArcLLMResult>>,
    pub llm_errors: FastMap<String, ArcLLMError>,

    // Tópicos Context (2)
    pub context_snapshots: FastMap<String, ArcContextSnapshot>,
    pub context_updates: FastMap<String, Vec<ArcContextUpdate>>,

    // Tópicos ToolCall (1)
    pub tool_calls: FastMap<String, ArcToolCallRequest>,

    // Tópicos ExecutionTrace (1)
    pub execution_traces: FastMap<String, Vec<ArcExecutionTraceEvent>>,

    // Tópicos Security (2)
    pub security_snapshots: FastMap<String, ArcSecurityPolicySnapshot>,
    pub security_updates: FastMap<String, Vec<ArcSecurityPolicyUpdate>>,

    // Tópicos QoS (3)
    pub qos_routing: FastMap<String, ArcQoSRoutingProfile>,
    pub qos_metrics: FastMap<String, ArcQoSMetric>,
    pub qos_violations: FastMap<String, ArcQoSViolation>,
    pub discovery_events: FastMap<String, ArcDiscoveryEvent>,
}

impl TopicCaches {
    pub fn new() -> Self {
        Self::default()
    }

    /// Upsert monotônico de task: só substitui se `task` supera a versão atual.
    /// Retorna o Arc vencedor (novo ou o mantido).
    pub fn upsert_task(&self, task: Task) -> ArcTask {
        self.tasks
            .entry(task.task_id.clone())
            .and_modify(|cur| {
                if supersedes(&task, cur) {
                    *cur = Arc::new(task.clone());
                }
            })
            .or_insert_with(|| Arc::new(task))
            .clone()
    }

    /// Upsert de agente: vence o maior `last_update_ns` (heartbeat mais recente).
    pub fn upsert_agent(&self, state: AgentState) -> ArcAgentState {
        self.agents
            .entry(state.agent_id.clone())
            .and_modify(|cur| {
                if state.last_update_ns >= cur.last_update_ns {
                    *cur = Arc::new(state.clone());
                }
            })
            .or_insert_with(|| Arc::new(state))
            .clone()
    }

    /// Append de output com dedup por `(task_id, seq_num)` (reentrega DDS não duplica).
    pub fn push_output(&self, output: TaskOutput) -> ArcTaskOutput {
        let arc = Arc::new(output);
        let mut entry = self.outputs.entry(arc.task_id.clone()).or_default();
        if let Some(existing) = entry.iter_mut().find(|o| o.seq_num == arc.seq_num) {
            if arc.emitted_at_ns >= existing.emitted_at_ns {
                *existing = arc.clone();
            }
        } else {
            entry.push(arc.clone());
        }
        arc
    }

    pub fn read_task(&self, task_id: &str) -> Option<ArcTask> {
        self.tasks.get(task_id).map(|t| t.clone())
    }

    pub fn all_tasks(&self) -> Vec<ArcTask> {
        self.tasks.iter().map(|t| t.clone()).collect()
    }

    pub fn read_agent(&self, agent_id: &str) -> Option<ArcAgentState> {
        self.agents.get(agent_id).map(|a| a.clone())
    }

    pub fn all_agents(&self) -> Vec<ArcAgentState> {
        self.agents.iter().map(|a| a.clone()).collect()
    }

    pub fn outputs_of(&self, task_id: &str) -> Vec<ArcTaskOutput> {
        self.outputs
            .get(task_id)
            .map(|o| o.clone())
            .unwrap_or_default()
    }

    // ── LLM caches ──────────────────────────────────────────────────────

    pub fn upsert_llm_request(&self, req: LLMInferenceRequest) -> ArcLLMRequest {
        self.llm_requests
            .entry(req.request_id.clone())
            .or_insert_with(|| Arc::new(req))
            .clone()
    }

    pub fn push_llm_result(&self, result: LLMInferenceResult) -> ArcLLMResult {
        let arc = Arc::new(result);
        let mut entry = self.llm_results.entry(arc.request_id.clone()).or_default();
        if let Some(existing) = entry.iter_mut().find(|r| r.seq_num == arc.seq_num) {
            if arc.emitted_at_ns >= existing.emitted_at_ns {
                *existing = arc.clone();
            }
        } else {
            entry.push(arc.clone());
        }
        arc
    }

    pub fn upsert_llm_error(&self, error: LLMInferenceError) -> ArcLLMError {
        self.llm_errors
            .entry(error.request_id.clone())
            .or_insert_with(|| Arc::new(error))
            .clone()
    }

    pub fn llm_results_of(&self, request_id: &str) -> Vec<ArcLLMResult> {
        self.llm_results
            .get(request_id)
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    // ── Context caches ──────────────────────────────────────────────────

    pub fn upsert_context_snapshot(&self, snap: ContextSnapshot) -> ArcContextSnapshot {
        self.context_snapshots
            .entry(snap.context_id.clone())
            .and_modify(|cur| {
                if snap.updated_at_ns >= cur.updated_at_ns {
                    *cur = Arc::new(snap.clone());
                }
            })
            .or_insert_with(|| Arc::new(snap))
            .clone()
    }

    pub fn push_context_update(&self, update: ContextUpdate) -> ArcContextUpdate {
        let arc = Arc::new(update);
        let mut entry = self
            .context_updates
            .entry(arc.context_id.clone())
            .or_default();
        entry.push(arc.clone());
        arc
    }

    // ── ToolCall cache ──────────────────────────────────────────────────

    pub fn upsert_tool_call(&self, call: ToolCallRequest) -> ArcToolCallRequest {
        self.tool_calls
            .entry(call.call_id.clone())
            .and_modify(|cur| {
                if call.created_at_ns >= cur.created_at_ns {
                    *cur = Arc::new(call.clone());
                }
            })
            .or_insert_with(|| Arc::new(call))
            .clone()
    }

    // ── ExecutionTrace cache ────────────────────────────────────────────

    pub fn push_execution_trace(&self, event: ExecutionTraceEvent) -> ArcExecutionTraceEvent {
        let arc = Arc::new(event);
        let mut entry = self
            .execution_traces
            .entry(arc.trace_id.clone())
            .or_default();
        if let Some(existing) = entry.iter_mut().find(|e| e.seq_num == arc.seq_num) {
            if arc.timestamp_ns >= existing.timestamp_ns {
                *existing = arc.clone();
            }
        } else {
            entry.push(arc.clone());
        }
        arc
    }

    // ── Security caches ─────────────────────────────────────────────────

    pub fn upsert_security_snapshot(
        &self,
        snap: SecurityPolicySnapshot,
    ) -> ArcSecurityPolicySnapshot {
        self.security_snapshots
            .entry(snap.policy_id.clone())
            .and_modify(|cur| {
                if snap.timestamp_ns >= cur.timestamp_ns {
                    *cur = Arc::new(snap.clone());
                }
            })
            .or_insert_with(|| Arc::new(snap))
            .clone()
    }

    pub fn push_security_update(&self, update: SecurityPolicyUpdate) -> ArcSecurityPolicyUpdate {
        let arc = Arc::new(update);
        let mut entry = self
            .security_updates
            .entry(arc.policy_id.clone())
            .or_default();
        entry.push(arc.clone());
        arc
    }

    // ── QoS caches ──────────────────────────────────────────────────────

    pub fn upsert_qos_routing(&self, profile: QoSRoutingProfile) -> ArcQoSRoutingProfile {
        self.qos_routing
            .entry(profile.profile_id.clone())
            .and_modify(|cur| {
                if profile.timestamp_ns >= cur.timestamp_ns {
                    *cur = Arc::new(profile.clone());
                }
            })
            .or_insert_with(|| Arc::new(profile))
            .clone()
    }

    pub fn upsert_qos_metric(&self, metric: QoSMetric) -> ArcQoSMetric {
        self.qos_metrics
            .entry(metric.metric_id.clone())
            .or_insert_with(|| Arc::new(metric))
            .clone()
    }

    pub fn upsert_qos_violation(&self, violation: QoSViolation) -> ArcQoSViolation {
        self.qos_violations
            .entry(violation.violation_id.clone())
            .or_insert_with(|| Arc::new(violation))
            .clone()
    }

    pub fn upsert_discovery_event(&self, event: DiscoveryEvent) -> ArcDiscoveryEvent {
        self.discovery_events
            .entry(event.event_id.clone())
            .or_insert_with(|| Arc::new(event))
            .clone()
    }
}

/// `new` supera `cur`? Regra (espelha o `_tasks_cache` do dds_backend Python):
///
/// 1. **Regressão de estado → rejeita** (status para trás ou `assigned_agent`
///    preenchido→vazio; `retry_count` maior sempre vence).
/// 2. **Sem regressão → o incoming vence** (last-write-wins por chegada).
///    É assim que a arbitragem de Exclusive Ownership do mesh se reflete nos
///    caches dos dois lados: o vencedor (menor GUID em empate de strength)
///    chega por último e sobrescreve. Usar "maior timestamp" aqui quebraria a
///    arbitragem (cada lado manteria o seu claim → execução dupla).
fn supersedes(new: &Task, cur: &Task) -> bool {
    !is_regression(new, cur)
}

/// Regressão de estado (equivalente ao `_detect_state_regression` do Python).
fn is_regression(new: &Task, cur: &Task) -> bool {
    if new.retry_count > cur.retry_count {
        return false; // retry vence sempre
    }
    if new.retry_count < cur.retry_count {
        return true;
    }
    // assigned_agent preenchido no cache, vazio no incoming → regressão
    if !cur.assigned_agent.is_empty() && new.assigned_agent.is_empty() {
        return true;
    }
    // status avançou; incoming quer voltar → regressão
    new.status < cur.status
}
