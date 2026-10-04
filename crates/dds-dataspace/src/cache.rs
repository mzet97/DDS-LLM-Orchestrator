//! Caches concorrentes por tópico (T-303, REQ-304/REQ-306).
//!
//! - `Arc<T>` imutável: leitores recebem snapshot consistente, sem lock de leitura.
//! - `DashMap` sharded: escrita de task não serializa com leitura de agente.
//! - **Regressão bloqueada por construção, sem monotonia de timestamp**: os
//!   upserts rejeitam *regressão de estado* (ver [`supersedes`]) e, FORA de
//!   regressão, aplicam **last-write-wins por chegada** — deliberado, porque é
//!   assim que a arbitragem de Exclusive Ownership do mesh se reflete nos
//!   caches dos dois lados (o vencedor chega por último e sobrescreve; usar
//!   "maior timestamp" faria cada lado manter o próprio claim → execução
//!   dupla). Ver o doc de [`supersedes`] e a revisão T-820-06/P3 (o header
//!   antigo dizia "empate pelo maior timestamp", o que contradizia o corpo).
//! - **Admissão explícita (T-820-06, generalização do RUST-CACHE-006)**: todo
//!   upsert devolve `(Arc<T>, bool)` — o `bool` diz se a amostra está DE FATO
//!   no cache (recuperável via `read_*`). Streams só entregam amostras
//!   aceitas; entregar rejeitadas reproduz o sintoma que motivou o fix de
//!   tasks (consumidor recebe algo que `read_*` nunca devolve), replicado
//!   antes nos outros 17 tópicos.

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
/// `pub(crate)`: fonte única para o cap espelhado no mock (`in_memory.rs`,
/// T-820-07) — os dois lados devem evoluir juntos.
pub(crate) const MAX_CHUNKS_PER_KEY: usize = 256;

/// Máximo de tasks no cache principal. Evita OOM sob carga sustentada.
/// Tasks além deste limite são rejeitadas (melhor que crashar).
/// Cap "soft": a checagem ocorre antes do `entry()` para não segurar o
/// guard de shard chamando `len()` (deadlock) — sob corrida de primeiras
/// inserções o mapa pode exceder o limite por poucas entradas, o que é
/// aceitável para um limite de proteção de memória.
/// `pub(crate)`: fonte única para o cap espelhado no mock (T-820-07).
pub(crate) const MAX_TASKS_IN_CACHE: usize = 2048;

/// TTL de tasks terminais quando o cache está sob pressão (cap atingido):
/// antes de rejeitar uma task nova, o upsert tenta aliviar terminais com
/// mais de 30 s (mesmo valor usado pelo sweeper do orquestrador).
/// `pub(crate)`: o mock `InMemoryDataSpace` reusa a mesma semântica para
/// espelhar o comportamento do cache real (T-820-07).
pub(crate) const TERMINAL_TTL_UNDER_PRESSURE: std::time::Duration =
    std::time::Duration::from_secs(30);

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
    /// Rejeições de admissão nos DEMAIS caches (T-820-06) — qualquer upsert
    /// que devolve `(_, false)` incrementa aqui, para que a perda não seja
    /// silenciosa como era antes da generalização do RUST-CACHE-006.
    rejections: AtomicU64,
}

impl TopicCaches {
    pub fn new() -> Self {
        Self::default()
    }

    /// Contabiliza uma rejeição de admissão nos caches fora o de tasks e
    /// devolve o Arc rejeitado (o chamador decide descartar; streams NÃO
    /// entregam — T-820-06).
    fn reject<T>(&self, arc: Arc<T>) -> Arc<T> {
        let total = self.rejections.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::warn!(
            rejected_total = total,
            "cache admission: amostra rejeitada (cap)"
        );
        arc
    }

    /// Total de rejeições de admissão fora o cache de tasks (que tem seus
    /// próprios contadores em [`Self::task_cache_stats`]).
    pub fn cache_rejections(&self) -> u64 {
        self.rejections.load(Ordering::Relaxed)
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
    ///
    /// Retorna `(Arc vencedor, accepted)` — `accepted == false` significa
    /// cache saturado e a amostra NÃO está legível via `read_agent`
    /// (T-820-06: o chamador/stream não deve entregá-la).
    pub fn upsert_agent(&self, state: AgentState) -> (ArcAgentState, bool) {
        if !cache_accepts_key(&self.agents, &state.agent_id) {
            return (self.reject(Arc::new(state)), false);
        }
        let accepted = true;
        (
            self.agents
                .entry(state.agent_id.clone())
                .and_modify(|cur| {
                    if state.last_update_ns >= cur.last_update_ns {
                        *cur = Arc::new(state.clone());
                    }
                })
                .or_insert_with(|| Arc::new(state))
                .clone(),
            accepted,
        )
    }

    /// Append de output com dedup por `(task_id, seq_num)` (reentrega DDS não duplica).
    /// Limita a `MAX_CHUNKS_PER_KEY` entradas por task para evitar OOM.
    ///
    /// Retorna `(Arc, accepted)` — `accepted == false` quando o cap do cache
    /// rejeitou a amostra (não legível via `outputs_of`; T-820-06).
    pub fn push_output(&self, output: TaskOutput) -> (ArcTaskOutput, bool) {
        let arc = Arc::new(output);
        if !cache_accepts_key(&self.outputs, &arc.task_id) {
            return (self.reject(arc), false);
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
        (arc, true)
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

    pub fn read_tool_call(&self, call_id: &str) -> Option<ArcToolCallRequest> {
        self.tool_calls.get(call_id).map(|c| c.clone())
    }

    // ── LLM caches ──────────────────────────────────────────────────────

    /// Upsert first-write-wins de request LLM (dedup por `request_id`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_llm_request(&self, req: LLMInferenceRequest) -> (ArcLLMRequest, bool) {
        if let Some(existing) = self.llm_requests.get(&req.request_id) {
            return (existing.clone(), true);
        }
        if self.llm_requests.len() >= MAX_TASKS_IN_CACHE {
            return (self.reject(Arc::new(req)), false);
        }
        (
            self.llm_requests
                .entry(req.request_id.clone())
                .or_insert_with(|| Arc::new(req))
                .clone(),
            true,
        )
    }

    /// Append de chunk de resultado LLM com dedup por `(request_id, seq_num)`.
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn push_llm_result(&self, result: LLMInferenceResult) -> (ArcLLMResult, bool) {
        let arc = Arc::new(result);
        if !cache_accepts_key(&self.llm_results, &arc.request_id) {
            return (self.reject(arc), false);
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
        (arc, true)
    }

    /// Upsert de erro LLM (primeiro vence por `request_id`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_llm_error(&self, error: LLMInferenceError) -> (ArcLLMError, bool) {
        if !cache_accepts_key(&self.llm_errors, &error.request_id) {
            return (self.reject(Arc::new(error)), false);
        }
        (
            self.llm_errors
                .entry(error.request_id.clone())
                .or_insert_with(|| Arc::new(error))
                .clone(),
            true,
        )
    }

    pub fn llm_results_of(&self, request_id: &str) -> Vec<ArcLLMResult> {
        self.llm_results
            .get(request_id)
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    // ── Context caches ──────────────────────────────────────────────────

    /// Upsert de snapshot de contexto (vence o maior `updated_at_ns`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_context_snapshot(&self, snap: ContextSnapshot) -> (ArcContextSnapshot, bool) {
        if !cache_accepts_key(&self.context_snapshots, &snap.context_id) {
            return (self.reject(Arc::new(snap)), false);
        }
        (
            self.context_snapshots
                .entry(snap.context_id.clone())
                .and_modify(|cur| {
                    if snap.updated_at_ns >= cur.updated_at_ns {
                        *cur = Arc::new(snap.clone());
                    }
                })
                .or_insert_with(|| Arc::new(snap))
                .clone(),
            true,
        )
    }

    /// Append de update de contexto (sem dedup — tópico Volatile de deltas).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn push_context_update(&self, update: ContextUpdate) -> (ArcContextUpdate, bool) {
        let arc = Arc::new(update);
        if !cache_accepts_key(&self.context_updates, &arc.context_id) {
            return (self.reject(arc), false);
        }
        let mut entry = self
            .context_updates
            .entry(arc.context_id.clone())
            .or_default();
        if entry.len() >= MAX_CHUNKS_PER_KEY {
            entry.remove(0);
        }
        entry.push(arc.clone());
        (arc, true)
    }

    // ── ToolCall cache ──────────────────────────────────────────────────

    /// Upsert de tool call com guard de regressão de status
    /// ([`call_supersedes_tool_call`]).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_tool_call(&self, call: ToolCallRequest) -> (ArcToolCallRequest, bool) {
        if !cache_accepts_key(&self.tool_calls, &call.call_id) {
            return (self.reject(Arc::new(call)), false);
        }
        (
            self.tool_calls
                .entry(call.call_id.clone())
                .and_modify(|cur| {
                    if call_supersedes_tool_call(&call, cur) {
                        *cur = Arc::new(call.clone());
                    }
                })
                .or_insert_with(|| Arc::new(call))
                .clone(),
            true,
        )
    }

    // ── ExecutionTrace cache ────────────────────────────────────────────

    /// Append de evento de trace com dedup por `(trace_id, seq_num)`.
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn push_execution_trace(
        &self,
        event: ExecutionTraceEvent,
    ) -> (ArcExecutionTraceEvent, bool) {
        let arc = Arc::new(event);
        if !cache_accepts_key(&self.execution_traces, &arc.trace_id) {
            return (self.reject(arc), false);
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
        (arc, true)
    }

    // ── Security caches ─────────────────────────────────────────────────

    /// Upsert de snapshot de política (vence o maior `timestamp_ns`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_security_snapshot(
        &self,
        snap: SecurityPolicySnapshot,
    ) -> (ArcSecurityPolicySnapshot, bool) {
        if !cache_accepts_key(&self.security_snapshots, &snap.policy_id) {
            return (self.reject(Arc::new(snap)), false);
        }
        (
            self.security_snapshots
                .entry(snap.policy_id.clone())
                .and_modify(|cur| {
                    if snap.timestamp_ns >= cur.timestamp_ns {
                        *cur = Arc::new(snap.clone());
                    }
                })
                .or_insert_with(|| Arc::new(snap))
                .clone(),
            true,
        )
    }

    /// Append de update de política (sem dedup — é um delta versionado).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn push_security_update(
        &self,
        update: SecurityPolicyUpdate,
    ) -> (ArcSecurityPolicyUpdate, bool) {
        let arc = Arc::new(update);
        if !cache_accepts_key(&self.security_updates, &arc.policy_id) {
            return (self.reject(arc), false);
        }
        let mut entry = self
            .security_updates
            .entry(arc.policy_id.clone())
            .or_default();
        if entry.len() >= MAX_CHUNKS_PER_KEY {
            entry.remove(0);
        }
        entry.push(arc.clone());
        (arc, true)
    }

    // ── QoS caches ──────────────────────────────────────────────────────

    /// Upsert de perfil de roteamento (vence o maior `timestamp_ns`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_qos_routing(&self, profile: QoSRoutingProfile) -> (ArcQoSRoutingProfile, bool) {
        if !cache_accepts_key(&self.qos_routing, &profile.profile_id) {
            return (self.reject(Arc::new(profile)), false);
        }
        (
            self.qos_routing
                .entry(profile.profile_id.clone())
                .and_modify(|cur| {
                    if profile.timestamp_ns >= cur.timestamp_ns {
                        *cur = Arc::new(profile.clone());
                    }
                })
                .or_insert_with(|| Arc::new(profile))
                .clone(),
            true,
        )
    }

    /// Upsert de métrica QoS (primeiro vence por `metric_id`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_qos_metric(&self, metric: QoSMetric) -> (ArcQoSMetric, bool) {
        if !cache_accepts_key(&self.qos_metrics, &metric.metric_id) {
            return (self.reject(Arc::new(metric)), false);
        }
        (
            self.qos_metrics
                .entry(metric.metric_id.clone())
                .or_insert_with(|| Arc::new(metric))
                .clone(),
            true,
        )
    }

    /// Upsert de violação QoS (primeiro vence por `violation_id`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_qos_violation(&self, violation: QoSViolation) -> (ArcQoSViolation, bool) {
        if !cache_accepts_key(&self.qos_violations, &violation.violation_id) {
            return (self.reject(Arc::new(violation)), false);
        }
        (
            self.qos_violations
                .entry(violation.violation_id.clone())
                .or_insert_with(|| Arc::new(violation))
                .clone(),
            true,
        )
    }

    /// Upsert de evento de discovery (primeiro vence por `event_id`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_discovery_event(&self, event: DiscoveryEvent) -> (ArcDiscoveryEvent, bool) {
        if !cache_accepts_key(&self.discovery_events, &event.event_id) {
            return (self.reject(Arc::new(event)), false);
        }
        (
            self.discovery_events
                .entry(event.event_id.clone())
                .or_insert_with(|| Arc::new(event))
                .clone(),
            true,
        )
    }

    /// Upsert de métrica de sistema. A chave do cache é o par composto
    /// `"metric_name:component_id"` (mesma chave de [`Self::read_system_metric`]) —
    /// a checagem de cap ANTES usava só `metric_name` e desligava a admissão
    /// de métricas de componentes novos quando um nome antigo saturava
    /// (T-820-06).
    pub fn upsert_system_metric(&self, metric: SystemMetric) -> (ArcSystemMetric, bool) {
        let key = format!("{}:{}", metric.metric_name, metric.component_id);
        if !cache_accepts_key(&self.system_metrics, &key) {
            return (self.reject(Arc::new(metric)), false);
        }
        (
            self.system_metrics
                .entry(key)
                .and_modify(|cur| {
                    if metric.timestamp_ns >= cur.timestamp_ns {
                        *cur = Arc::new(metric.clone());
                    }
                })
                .or_insert_with(|| Arc::new(metric))
                .clone(),
            true,
        )
    }

    pub fn read_system_metric(
        &self,
        metric_name: &str,
        component_id: &str,
    ) -> Option<ArcSystemMetric> {
        let key = format!("{metric_name}:{component_id}");
        self.system_metrics.get(&key).map(|m| m.clone())
    }

    /// Upsert de status de servidor (last-write-wins por `server_id`).
    /// Retorna `(Arc, accepted)` — ver nota de admissão em [`TopicCaches::upsert_agent`].
    pub fn upsert_server_status(&self, status: ServerStatus) -> (ArcServerStatus, bool) {
        if !cache_accepts_key(&self.server_status, &status.server_id) {
            return (self.reject(Arc::new(status)), false);
        }
        (
            self.server_status
                .entry(status.server_id.clone())
                .and_modify(|cur| *cur = Arc::new(status.clone()))
                .or_insert_with(|| Arc::new(status))
                .clone(),
            true,
        )
    }

    pub fn read_server_status(&self, server_id: &str) -> Option<ArcServerStatus> {
        self.server_status.get(server_id).map(|s| s.clone())
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
                orch_common::TaskStatus::is_terminal_i32(t.status)
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
            // de uma campanha de horas — achado real da Rodada 6/7. Este era
            // o único mapa que o método não removia, apesar do próprio
            // `terminal_ids` ser computado a partir dele.
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
        total += self.system_metrics.len();
        total += self.server_status.len();
        total
    }

    /// Limpa TODOS os caches de uma vez (`DataSpaceApi::shutdown` — sem a
    /// duplicação de `clear()`s que escondia os mapas re-limpados).
    pub fn clear_all(&self) {
        self.tasks.clear();
        self.agents.clear();
        self.outputs.clear();
        self.system_metrics.clear();
        self.server_status.clear();
        self.llm_requests.clear();
        self.llm_results.clear();
        self.llm_errors.clear();
        self.context_snapshots.clear();
        self.context_updates.clear();
        self.tool_calls.clear();
        self.execution_traces.clear();
        self.security_snapshots.clear();
        self.security_updates.clear();
        self.qos_routing.clear();
        self.qos_metrics.clear();
        self.qos_violations.clear();
        self.discovery_events.clear();
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
///
/// `pub(crate)`: reusado pelo mock `InMemoryDataSpace` para espelhar o filtro
/// de regressão do cache real (T-820-07) — os dois lados não podem divergir.
pub(crate) fn supersedes(new: &Task, cur: &Task) -> bool {
    !is_regression(new, cur)
}

/// Regressão de estado (equivalente ao `_detect_state_regression` do Python).
pub(crate) fn is_regression(new: &Task, cur: &Task) -> bool {
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

/// Terminais do fluxo de tool call no canon (`models.py::ToolCallStatus`:
/// DENIED/COMPLETED/FAILED = 2|4|5) — resolvidos pelo tipo de `orch-common`
/// em vez de magic numbers (T-820-06/P3).
fn is_call_terminal(status: i32) -> bool {
    orch_common::ToolCallStatus::is_terminal_i32(status)
}
