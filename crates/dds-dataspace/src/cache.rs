//! Caches concorrentes por tópico (T-303, REQ-304/REQ-306).
//!
//! - `Arc<T>` imutável: leitores recebem snapshot consistente, sem lock de leitura.
//! - `DashMap` sharded: escrita de task não serializa com leitura de agente.
//! - **Regressão bloqueada por construção**: `upsert_task` só substitui se a nova
//!   versão *supera* a atual (status monotônico; empate pelo maior timestamp) —
//!   elimina as guardas anti-regressão (C1) do `dds_backend` Python.

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, ContextSnapshot, ContextUpdate, DiscoveryEvent, ExecutionTraceEvent, QoSMetric,
    QoSRoutingProfile, QoSViolation, SecurityPolicySnapshot, SecurityPolicyUpdate, SystemMetric,
    Task, TaskOutput, ToolCallRequest,
};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult, ServerStatus,
};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Máximo de chunks por task em caches Vec (outputs, llm_results, etc.).
/// Evita crescimento indefinido sob carga sustentada.
const MAX_CHUNKS_PER_KEY: usize = 256;

/// Máximo de tasks no cache principal. Evita OOM sob carga sustentada.
/// Tasks além deste limite são rejeitadas (melhor que crashar).
/// Cap "soft": a checagem ocorre antes do `entry()` para não segurar o
/// guard de shard chamando `len()` (deadlock) — sob corrida de primeiras
/// inserções o mapa pode exceder o limite por poucas entradas, o que é
/// aceitável para um limite de proteção de memória.
const MAX_TASKS_IN_CACHE: usize = 2048;

/// TTL de tasks terminais quando o cache está sob pressão (cap atingido):
/// antes de rejeitar uma task nova, o upsert tenta aliviar terminais com
/// mais de 30 s (mesmo valor usado pelo sweeper do orquestrador).
const TERMINAL_TTL_UNDER_PRESSURE: std::time::Duration = std::time::Duration::from_secs(30);

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
pub type ArcSystemMetric = Arc<SystemMetric>;
pub type ArcServerStatus = Arc<ServerStatus>;

/// Resultado explícito do upsert de task (RUST-CACHE-006).
///
/// Antes desta revisão, `upsert_task` retornava `Arc<Task>` mesmo quando o
/// cache estava cheio e a amostra NÃO tinha sido inserida — o `stream_tasks`
/// entregava uma task que `read_task`/`confirm_ownership` jamais encontrariam,
/// fazendo claims válidos falharem permanentemente após a saturação.
pub enum TaskUpsert {
    /// A amostra está no cache (inserida, substituída ou mantida): o Arc
    /// retornado é o conteúdo vencedor e é imediatamente legível via
    /// `read_task`.
    Accepted(ArcTask),
    /// Cache saturado mesmo após eviction de terminais: a amostra NÃO está
    /// no cache. O chamador não deve entregá-la ao consumidor.
    Rejected(ArcTask),
}

impl TaskUpsert {
    /// `true` quando a amostra entregue é recuperável do cache.
    pub fn is_accepted(&self) -> bool {
        matches!(self, TaskUpsert::Accepted(_))
    }

    /// Consome o resultado e devolve o Arc (vencedor ou rejeitado).
    pub fn into_arc(self) -> ArcTask {
        match self {
            TaskUpsert::Accepted(t) | TaskUpsert::Rejected(t) => t,
        }
    }
}

impl Deref for TaskUpsert {
    type Target = ArcTask;
    fn deref(&self) -> &ArcTask {
        match self {
            TaskUpsert::Accepted(t) | TaskUpsert::Rejected(t) => t,
        }
    }
}

fn cache_accepts_key<V>(cache: &FastMap<String, V>, key: &str) -> bool {
    cache.contains_key(key) || cache.len() < MAX_TASKS_IN_CACHE
}

/// Caches do DataSpace (um por processo).
#[derive(Default)]
pub struct TopicCaches {
    // Tópicos originais (3)
    pub tasks: FastMap<String, ArcTask>,
    pub agents: FastMap<String, ArcAgentState>,
    pub outputs: FastMap<String, Vec<ArcTaskOutput>>,

    // Runtime telemetry (2)
    pub system_metrics: FastMap<String, ArcSystemMetric>,
    pub server_status: FastMap<String, ArcServerStatus>,

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

    // Contadores de pressão do cache de tasks (RUST-CACHE-006).
    tasks_rejected: AtomicU64,
    tasks_evicted: AtomicU64,
}

impl TopicCaches {
    pub fn new() -> Self {
        Self::default()
    }

    /// Upsert monotônico de task: só substitui se `task` supera a versão atual.
    /// Retorna o resultado explícito ([`TaskUpsert`]) — `Accepted` garante que o
    /// Arc retornado é o conteúdo do cache (legível via `read_task`).
    ///
    /// Atômico por `task_id` (RUST-CACHE-006B): a decisão `supersedes` roda
    /// dentro da única operação `entry()` (Occupied/Vacant), sob o guard do
    /// shard — na versão anterior (`get_mut` seguido de `or_insert`), duas
    /// primeiras-inserções concorrentes podiam deixar a versão mais fraca
    /// vencer sem passar por `supersedes`.
    ///
    /// Sob pressão (cap), tenta eviction de terminais antes de rejeitar
    /// (RUST-CACHE-006): o cache volta a aceitar tasks novas depois que
    /// terminais antigas saem.
    pub fn upsert_task(&self, task: Task) -> TaskUpsert {
        // Cap soft verificado ANTES do entry(): `len()` trava todos os shards
        // e não pode ser chamado segurando o guard de um shard (deadlock).
        if !self.tasks.contains_key(&task.task_id) && self.tasks.len() >= MAX_TASKS_IN_CACHE {
            // Evict-before-reject: alivia terminais antigas e re-checa.
            self.evict_terminal_tasks(TERMINAL_TTL_UNDER_PRESSURE);
            if self.tasks.len() >= MAX_TASKS_IN_CACHE {
                let rejected_total = self.tasks_rejected.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!(
                    cache_size = self.tasks.len(),
                    rejected_total,
                    "task cache full, rejecting new task (NOT delivered downstream)"
                );
                return TaskUpsert::Rejected(Arc::new(task));
            }
        }
        match self.tasks.entry(task.task_id.clone()) {
            Entry::Occupied(mut e) => {
                if supersedes(&task, e.get()) {
                    *e.get_mut() = Arc::new(task);
                }
                TaskUpsert::Accepted(Arc::clone(e.get()))
            }
            Entry::Vacant(e) => TaskUpsert::Accepted(Arc::clone(&e.insert(Arc::new(task)))),
        }
    }

    /// Upsert de agente: vence o maior `last_update_ns` (heartbeat mais recente).
    pub fn upsert_agent(&self, state: AgentState) -> ArcAgentState {
        if !cache_accepts_key(&self.agents, &state.agent_id) {
            return Arc::new(state);
        }
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
    /// Limita a `MAX_CHUNKS_PER_KEY` entradas por task para evitar OOM.
    pub fn push_output(&self, output: TaskOutput) -> ArcTaskOutput {
        let arc = Arc::new(output);
        if !cache_accepts_key(&self.outputs, &arc.task_id) {
            return arc;
        }
        let mut entry = self.outputs.entry(arc.task_id.clone()).or_default();
        if let Some(existing) = entry.iter_mut().find(|o| o.seq_num == arc.seq_num) {
            if arc.emitted_at_ns >= existing.emitted_at_ns {
                *existing = arc.clone();
            }
        } else {
            if entry.len() >= MAX_CHUNKS_PER_KEY {
                entry.remove(0);
            }
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

    pub fn upsert_system_metric(&self, metric: SystemMetric) -> ArcSystemMetric {
        let key = format!("{}\u{1f}{}", metric.metric_name, metric.component_id);
        self.system_metrics
            .entry(key)
            .and_modify(|current| {
                if metric.timestamp_ns >= current.timestamp_ns {
                    *current = Arc::new(metric.clone());
                }
            })
            .or_insert_with(|| Arc::new(metric))
            .clone()
    }

    pub fn read_system_metric(
        &self,
        metric_name: &str,
        component_id: &str,
    ) -> Option<ArcSystemMetric> {
        let key = format!("{metric_name}\u{1f}{component_id}");
        self.system_metrics.get(&key).map(|metric| metric.clone())
    }

    pub fn upsert_server_status(&self, status: ServerStatus) -> ArcServerStatus {
        let status = Arc::new(status);
        self.server_status
            .insert(status.server_id.clone(), Arc::clone(&status));
        status
    }

    pub fn read_server_status(&self, server_id: &str) -> Option<ArcServerStatus> {
        self.server_status
            .get(server_id)
            .map(|status| status.clone())
    }

    // ── LLM caches ──────────────────────────────────────────────────────

    pub fn upsert_llm_request(&self, req: LLMInferenceRequest) -> ArcLLMRequest {
        if let Some(existing) = self.llm_requests.get(&req.request_id) {
            return existing.clone();
        }
        if self.llm_requests.len() >= MAX_TASKS_IN_CACHE {
            return Arc::new(req);
        }
        self.llm_requests
            .entry(req.request_id.clone())
            .or_insert_with(|| Arc::new(req))
            .clone()
    }

    pub fn push_llm_result(&self, result: LLMInferenceResult) -> ArcLLMResult {
        let arc = Arc::new(result);
        if !cache_accepts_key(&self.llm_results, &arc.request_id) {
            return arc;
        }
        let mut entry = self.llm_results.entry(arc.request_id.clone()).or_default();
        if let Some(existing) = entry.iter_mut().find(|r| r.seq_num == arc.seq_num) {
            if arc.emitted_at_ns >= existing.emitted_at_ns {
                *existing = arc.clone();
            }
        } else {
            if entry.len() >= MAX_CHUNKS_PER_KEY {
                entry.remove(0);
            }
            entry.push(arc.clone());
        }
        arc
    }

    pub fn upsert_llm_error(&self, error: LLMInferenceError) -> ArcLLMError {
        if !cache_accepts_key(&self.llm_errors, &error.request_id) {
            return Arc::new(error);
        }
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
        if !cache_accepts_key(&self.context_snapshots, &snap.context_id) {
            return Arc::new(snap);
        }
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
        if !cache_accepts_key(&self.context_updates, &arc.context_id) {
            return arc;
        }
        let mut entry = self
            .context_updates
            .entry(arc.context_id.clone())
            .or_default();
        if entry.len() >= MAX_CHUNKS_PER_KEY {
            entry.remove(0);
        }
        entry.push(arc.clone());
        arc
    }

    // ── ToolCall cache ──────────────────────────────────────────────────

    pub fn upsert_tool_call(&self, call: ToolCallRequest) -> ArcToolCallRequest {
        if !cache_accepts_key(&self.tool_calls, &call.call_id) {
            return Arc::new(call);
        }
        self.tool_calls
            .entry(call.call_id.clone())
            .and_modify(|cur| {
                if call_supersedes_tool_call(&call, cur) {
                    *cur = Arc::new(call.clone());
                }
            })
            .or_insert_with(|| Arc::new(call))
            .clone()
    }

    // ── ExecutionTrace cache ────────────────────────────────────────────

    pub fn push_execution_trace(&self, event: ExecutionTraceEvent) -> ArcExecutionTraceEvent {
        let arc = Arc::new(event);
        if !cache_accepts_key(&self.execution_traces, &arc.trace_id) {
            return arc;
        }
        let mut entry = self
            .execution_traces
            .entry(arc.trace_id.clone())
            .or_default();
        if let Some(existing) = entry.iter_mut().find(|e| e.seq_num == arc.seq_num) {
            if arc.timestamp_ns >= existing.timestamp_ns {
                *existing = arc.clone();
            }
        } else {
            if entry.len() >= MAX_CHUNKS_PER_KEY {
                entry.remove(0);
            }
            entry.push(arc.clone());
        }
        arc
    }

    // ── Security caches ─────────────────────────────────────────────────

    pub fn upsert_security_snapshot(
        &self,
        snap: SecurityPolicySnapshot,
    ) -> ArcSecurityPolicySnapshot {
        if !cache_accepts_key(&self.security_snapshots, &snap.policy_id) {
            return Arc::new(snap);
        }
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
        if !cache_accepts_key(&self.security_updates, &arc.policy_id) {
            return arc;
        }
        let mut entry = self
            .security_updates
            .entry(arc.policy_id.clone())
            .or_default();
        if entry.len() >= MAX_CHUNKS_PER_KEY {
            entry.remove(0);
        }
        entry.push(arc.clone());
        arc
    }

    // ── QoS caches ──────────────────────────────────────────────────────

    pub fn upsert_qos_routing(&self, profile: QoSRoutingProfile) -> ArcQoSRoutingProfile {
        if !cache_accepts_key(&self.qos_routing, &profile.profile_id) {
            return Arc::new(profile);
        }
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
        if !cache_accepts_key(&self.qos_metrics, &metric.metric_id) {
            return Arc::new(metric);
        }
        self.qos_metrics
            .entry(metric.metric_id.clone())
            .or_insert_with(|| Arc::new(metric))
            .clone()
    }

    pub fn upsert_qos_violation(&self, violation: QoSViolation) -> ArcQoSViolation {
        if !cache_accepts_key(&self.qos_violations, &violation.violation_id) {
            return Arc::new(violation);
        }
        self.qos_violations
            .entry(violation.violation_id.clone())
            .or_insert_with(|| Arc::new(violation))
            .clone()
    }

    pub fn upsert_discovery_event(&self, event: DiscoveryEvent) -> ArcDiscoveryEvent {
        if !cache_accepts_key(&self.discovery_events, &event.event_id) {
            return Arc::new(event);
        }
        self.discovery_events
            .entry(event.event_id.clone())
            .or_insert_with(|| Arc::new(event))
            .clone()
    }

    /// Remove dados associados a tasks em estado terminal (DONE/FAILED) completadas
    /// há mais de `max_age`. Evita crescimento indefinido dos caches Vec.
    pub fn evict_terminal_tasks(&self, max_age: std::time::Duration) {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let max_age_ns = max_age.as_nanos() as u64;

        let terminal_ids: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| {
                (t.status == 3 || t.status == 4)
                    && t.completed_at_ns > 0
                    && now_ns.saturating_sub(t.completed_at_ns) > max_age_ns
            })
            .map(|t| t.task_id.clone())
            .collect();

        for id in &terminal_ids {
            // `self.tasks` primeiro — sem isto, o mapa principal nunca
            // encolhe e todo scan de `all_tasks()` (ex.: `reap_dead_agents`,
            // rodando a cada ~2s) itera o histórico completo de TODA task já
            // vista pelo processo, ficando mais caro a cada ciclo ao longo
            // de uma campanha de horas — achado real da Rodada 6/7 (a

            // única coisa que este método já não removia, apesar do
            // próprio `terminal_ids` ser computado a partir dele).
            self.tasks.remove(id);
            self.outputs.remove(id);
            // LLM results/requests/errors keyed by request_id, not task_id.
            // They share the same UUID in the current codebase.
            self.llm_results.remove(id);
            self.llm_requests.remove(id);
            self.llm_errors.remove(id);
            self.context_updates.remove(id);
            self.execution_traces.remove(id);
            self.security_updates.remove(id);
        }

        if !terminal_ids.is_empty() {
            self.tasks_evicted
                .fetch_add(terminal_ids.len() as u64, Ordering::Relaxed);
            tracing::debug!(
                count = terminal_ids.len(),
                "cache eviction: terminal tasks cleaned"
            );
        }
    }

    /// Snapshot dos contadores de pressão do cache de tasks (RUST-CACHE-006):
    /// ocupação atual, rejeições por saturação e evictions de terminais.
    pub fn task_cache_stats(&self) -> TaskCacheStats {
        TaskCacheStats {
            tasks_len: self.tasks.len(),
            tasks_rejected: self.tasks_rejected.load(Ordering::Relaxed),
            tasks_evicted: self.tasks_evicted.load(Ordering::Relaxed),
        }
    }

    /// Número total de entradas em todos os caches Vec (outputs, results, etc.).
    /// Útil para monitoramento de pressão de memória.
    pub fn vec_cache_entries(&self) -> usize {
        let mut total = 0usize;
        for e in self.outputs.iter() {
            total += e.value().len();
        }
        for e in self.llm_results.iter() {
            total += e.value().len();
        }
        for e in self.context_updates.iter() {
            total += e.value().len();
        }
        for e in self.execution_traces.iter() {
            total += e.value().len();
        }
        for e in self.security_updates.iter() {
            total += e.value().len();
        }
        total
    }
}

/// Contadores de pressão do cache de tasks (ver [`TopicCaches::task_cache_stats`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCacheStats {
    pub tasks_len: usize,
    pub tasks_rejected: u64,
    pub tasks_evicted: u64,
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

fn call_supersedes_tool_call(new: &ToolCallRequest, cur: &ToolCallRequest) -> bool {
    if new.created_at_ns < cur.created_at_ns {
        return false;
    }
    if is_call_terminal(cur.status) {
        return false;
    }
    if is_call_terminal(new.status) {
        return true;
    }
    if new.status < cur.status {
        return false;
    }
    true
}

fn is_call_terminal(status: i32) -> bool {
    matches!(status, 2 | 4 | 5)
}
