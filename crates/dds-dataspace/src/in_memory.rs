//! InMemoryDataSpace — mock para testes (REQ-309, T-301).
//!
//! Implementa DataSpaceApi usando estruturas de dados em memória.
//! Usado para contract tests A/B (mesma bateria roda contra mock e DDS real).
//!
//! ## Divergências documentadas vs `DataSpace` real (T-820-07, P1-6 da
//! revisão 2026-10-04 — EXP1a precisa saber o que NÃO está sendo exercitado)
//!
//! Além da divergência de **ownership** (abaixo, no doc de `write_task`; o
//! mock não arbitra Exclusive Ownership por strength), o mock replica AGORA:
//! filtro de regressão de status (`cache::supersedes`), caps (2048 keys /
//! 256 chunks por key, com eviction de terminais sob pressão) e dedup de
//! outputs por `(task_id, seq_num)`. Divergências que PERMANECEM:
//!
//! 1. **Consistência imediata write→read**: o mock insere no cache
//!    sincronamente no `write_*`; o real é write-through-não/eventual — o
//!    cache só é alimentado pela stream (~ms depois, quando o pump é polled).
//!    Testes que assumem read-back instantâneo após write não validam o
//!    comportamento real.
//! 2. **Sem RHC/arbitragem no caminho de leitura**: `read_task` do real via
//!    mesh (RHC do reader) pode divergir do cache; o mock só tem o mapa.
//! 3. **Perfil de perda do broadcast ≠ RHC KeepLast**: o mock entrega por
//!    `tokio::broadcast` (1024); um consumidor lento recebe `Lagged` e PERDE
//!    o histórico todo de uma vez, enquanto o real (reader KeepLast(N)
//!    por tópico) perde as amostras mais antigas individualmente. Além disso
//!    o real com QoS TransientLocal re-entrega histórico para late joiners —
//!    o mock começa vazio para cada assinante.
//! 4. **`read_tool_call` cache vs mesh**: no real, `read_tool_call` lê o
//!    RHC do mesh (`read_tool_call_mesh`), que reflete a arbitragem de
//!    ownership; no mock lê o mapa inserido pelo último write.
//! 5. **Timing sem settle/pump**: a entrega no mock é síncrona via
//!    `broadcast` no próprio `write_*`; no real a amostra percorre o RHC →
//!    WaitSet compartilhado → driver task → stream (latência de ms e
//!    dependente do runtime estar polled). Testes que validam latência ou
//!    janelas de settle não transferem para o DDS real.
//!
//! (1)–(5) e a lista acima existem para que as conclusões A/B do EXP1a
//! isolem esses efeitos — ver a revisão de 2026-10-04 §2 item 6.

use crate::api::{DataSpaceApi, DataSpaceError};
use crate::cache;
use async_stream::stream;
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
use futures_core::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Cap "soft" de chaves compartilhado com o cache real (T-820-07): rejeita
/// inserção de chave NOVA quando o mapa está no limite (`cache::
/// MAX_TASKS_IN_CACHE`); chaves existentes sempre atualizam.
fn accepts_key<V>(map: &DashMap<String, V>, key: &str) -> bool {
    map.contains_key(key) || map.len() < cache::MAX_TASKS_IN_CACHE
}

/// Cap de chunks por chave compartilhado com o cache real (T-820-07):
/// descarta o chunk mais antigo ao inserir além de `cache::MAX_CHUNKS_PER_KEY`
/// (mesma política de `cache::push_*`).
fn push_capped<T>(vec: &mut Vec<T>, item: T) {
    if vec.len() >= cache::MAX_CHUNKS_PER_KEY {
        vec.remove(0);
    }
    vec.push(item);
}

/// Mock DataSpace em memória para testes.
///
/// `tasks`/`outputs` guardam `Arc<Task>`/`Arc<TaskOutput>` (Fase 3 do
/// `OPTIMIZATION_PLAN.md`) para espelhar o `DataSpace` real (`cache.rs` já
/// guardava `Arc` internamente; só o mock ainda clonava a struct inteira).
pub struct InMemoryDataSpace {
    // Tópicos originais
    tasks: DashMap<String, Arc<Task>>,
    agents: DashMap<String, AgentState>,
    outputs: DashMap<String, Vec<Arc<TaskOutput>>>,
    task_tx: broadcast::Sender<Arc<Task>>,
    agent_tx: broadcast::Sender<AgentState>,
    output_tx: broadcast::Sender<Arc<TaskOutput>>,

    system_metrics: DashMap<(String, String), SystemMetric>,
    server_status: DashMap<String, ServerStatus>,
    system_metric_tx: broadcast::Sender<SystemMetric>,
    server_status_tx: broadcast::Sender<ServerStatus>,

    // Tópicos LLM
    llm_requests: DashMap<String, LLMInferenceRequest>,
    llm_results: DashMap<String, Vec<LLMInferenceResult>>,
    llm_errors: DashMap<String, LLMInferenceError>,
    llm_request_tx: broadcast::Sender<LLMInferenceRequest>,
    llm_result_tx: broadcast::Sender<LLMInferenceResult>,
    llm_error_tx: broadcast::Sender<LLMInferenceError>,

    // Tópicos Context
    context_snapshots: DashMap<String, ContextSnapshot>,
    context_updates: DashMap<String, Vec<ContextUpdate>>,
    context_snapshot_tx: broadcast::Sender<ContextSnapshot>,
    context_update_tx: broadcast::Sender<ContextUpdate>,

    // Tópicos ToolCall
    tool_calls: DashMap<String, ToolCallRequest>,
    tool_call_tx: broadcast::Sender<ToolCallRequest>,

    // Tópicos ExecutionTrace
    execution_traces: DashMap<String, Vec<ExecutionTraceEvent>>,
    execution_trace_tx: broadcast::Sender<ExecutionTraceEvent>,

    // Tópicos Security
    security_snapshots: DashMap<String, SecurityPolicySnapshot>,
    security_updates: DashMap<String, Vec<SecurityPolicyUpdate>>,
    security_snapshot_tx: broadcast::Sender<SecurityPolicySnapshot>,
    security_update_tx: broadcast::Sender<SecurityPolicyUpdate>,

    // Tópicos QoS
    qos_routing: DashMap<String, QoSRoutingProfile>,
    qos_metrics: DashMap<String, QoSMetric>,
    qos_violations: DashMap<String, QoSViolation>,
    discovery_events: DashMap<String, DiscoveryEvent>,
    qos_routing_tx: broadcast::Sender<QoSRoutingProfile>,
    qos_metric_tx: broadcast::Sender<QoSMetric>,
    qos_violation_tx: broadcast::Sender<QoSViolation>,
    discovery_event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl Default for InMemoryDataSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDataSpace {
    pub fn new() -> Self {
        let (task_tx, _) = broadcast::channel(1024);
        let (agent_tx, _) = broadcast::channel(1024);
        let (output_tx, _) = broadcast::channel(1024);
        let (system_metric_tx, _) = broadcast::channel(1024);
        let (server_status_tx, _) = broadcast::channel(1024);
        let (llm_request_tx, _) = broadcast::channel(1024);
        let (llm_result_tx, _) = broadcast::channel(1024);
        let (llm_error_tx, _) = broadcast::channel(1024);
        let (context_snapshot_tx, _) = broadcast::channel(1024);
        let (context_update_tx, _) = broadcast::channel(1024);
        let (tool_call_tx, _) = broadcast::channel(1024);
        let (execution_trace_tx, _) = broadcast::channel(1024);
        let (security_snapshot_tx, _) = broadcast::channel(1024);
        let (security_update_tx, _) = broadcast::channel(1024);
        let (qos_routing_tx, _) = broadcast::channel(1024);
        let (qos_metric_tx, _) = broadcast::channel(1024);
        let (qos_violation_tx, _) = broadcast::channel(1024);
        let (discovery_event_tx, _) = broadcast::channel(1024);

        Self {
            tasks: DashMap::new(),
            agents: DashMap::new(),
            outputs: DashMap::new(),
            task_tx,
            agent_tx,
            output_tx,
            system_metrics: DashMap::new(),
            server_status: DashMap::new(),
            system_metric_tx,
            server_status_tx,

            llm_requests: DashMap::new(),
            llm_results: DashMap::new(),
            llm_errors: DashMap::new(),
            llm_request_tx,
            llm_result_tx,
            llm_error_tx,

            context_snapshots: DashMap::new(),
            context_updates: DashMap::new(),
            context_snapshot_tx,
            context_update_tx,

            tool_calls: DashMap::new(),
            tool_call_tx,

            execution_traces: DashMap::new(),
            execution_trace_tx,

            security_snapshots: DashMap::new(),
            security_updates: DashMap::new(),
            security_snapshot_tx,
            security_update_tx,

            qos_routing: DashMap::new(),
            qos_metrics: DashMap::new(),
            qos_violations: DashMap::new(),
            discovery_events: DashMap::new(),
            qos_routing_tx,
            qos_metric_tx,
            qos_violation_tx,
            discovery_event_tx,
        }
    }

    /// Espelho de `cache::TopicCaches::evict_terminal_tasks` (T-820-07):
    /// remove tasks terminais (DONE/FAILED) completadas há mais de `max_age`
    /// e os dados derivados delas — usado sob pressão do cap, antes de
    /// rejeitar, exatamente como o cache real.
    fn evict_terminal_tasks(&self, max_age: std::time::Duration) {
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
            self.tasks.remove(id);
            self.outputs.remove(id);
            // LLM results/requests/errors keyed by request_id, not task_id.
            // They share the same UUID in the current codebase (idem cache.rs).
            self.llm_results.remove(id);
            self.llm_requests.remove(id);
            self.llm_errors.remove(id);
            self.context_updates.remove(id);
            self.execution_traces.remove(id);
            self.security_updates.remove(id);
        }
    }
}

/// Macro para gerar implementações de subscribe repetitivas
macro_rules! impl_subscribe {
    ($fn_name:ident, $rx_field:ident, $item_type:ty) => {
        fn $fn_name(&self) -> Pin<Box<dyn Stream<Item = $item_type> + Send>> {
            let mut rx = self.$rx_field.subscribe();
            Box::pin(stream! {
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

#[async_trait::async_trait]
impl DataSpaceApi for InMemoryDataSpace {
    // === Tasks ===

    /// DIVERGÊNCIA DOCUMENTADA vs DDS real (H2, atualizada T-820-07): `Tasks`
    /// usa EXCLUSIVE ownership (STRENGTH_CLIENT=10 < STRENGTH_AGENT=100 <
    /// STRENGTH_ORCHESTRATOR=200) e o writer mais fraco perde a arbitragem.
    /// O mock não recebe strength por escrita (`DataSpaceApi::write_task`
    /// não tem o parâmetro) e não arbitra strength — para escritas NÃO
    /// regressivas aplica last-write-wins por chegada (como o cache real
    /// reflete o vencedor da arbitragem, que chega por último). O que o mock
    /// AGORA replica do cache real: filtro de regressão de status
    /// (`cache::supersedes`), cap de 2048 keys com eviction de terminais e
    /// não-entrega (sem broadcast) de amostra rejeitada — a stream real não
    /// entrega amostra que o cache recusou. Não usar o mock para validar a
    /// arbitragem de ownership em si (relevante para EXP1a; ver o header do
    /// módulo para as demais divergências).
    async fn write_task(&self, task: Task) -> Result<(), DataSpaceError> {
        let arc = Arc::new(task);
        // Cap soft com eviction de terminais (espelha `cache::upsert_task`).
        if !self.tasks.contains_key(&arc.task_id) && self.tasks.len() >= cache::MAX_TASKS_IN_CACHE {
            self.evict_terminal_tasks(cache::TERMINAL_TTL_UNDER_PRESSURE);
            if self.tasks.len() >= cache::MAX_TASKS_IN_CACHE {
                return Ok(()); // rejeitada: sem insert, sem broadcast
            }
        }
        // Filtro de regressão de status (espelha `cache::supersedes`).
        match self.tasks.entry(arc.task_id.clone()) {
            Entry::Occupied(mut e) => {
                if cache::supersedes(arc.as_ref(), e.get().as_ref()) {
                    e.insert(Arc::clone(&arc));
                } else {
                    return Ok(()); // regressão rejeitada: sem broadcast
                }
            }
            Entry::Vacant(e) => {
                e.insert(Arc::clone(&arc));
            }
        }
        let _ = self.task_tx.send(arc);
        Ok(())
    }

    /// Identico ao `write_task` no mock: o mock não modela ownership, então
    /// "sem assumir ownership" não muda nada aqui (paridade de API apenas).
    async fn write_task_without_ownership(&self, task: Task) -> Result<(), DataSpaceError> {
        self.write_task(task).await
    }

    async fn read_task(&self, task_id: &str) -> Result<Option<Arc<Task>>, DataSpaceError> {
        Ok(self.tasks.get(task_id).map(|t| t.clone()))
    }

    async fn all_tasks(&self) -> Result<Vec<Arc<Task>>, DataSpaceError> {
        Ok(self.tasks.iter().map(|t| t.clone()).collect())
    }

    impl_subscribe!(subscribe_tasks, task_tx, Arc<Task>);

    // === Agents ===

    async fn write_agent_state(&self, state: AgentState) -> Result<(), DataSpaceError> {
        // T-820-07: cap espelhado do cache real (rejeita chave nova quando cheio).
        if accepts_key(&self.agents, &state.agent_id) {
            self.agents.insert(state.agent_id.clone(), state.clone());
            let _ = self.agent_tx.send(state);
        }
        Ok(())
    }

    async fn read_agent_state(&self, agent_id: &str) -> Result<Option<AgentState>, DataSpaceError> {
        Ok(self.agents.get(agent_id).map(|a| a.clone()))
    }

    async fn all_agents(&self) -> Result<Vec<AgentState>, DataSpaceError> {
        Ok(self.agents.iter().map(|a| a.clone()).collect())
    }

    impl_subscribe!(subscribe_agent_states, agent_tx, AgentState);

    // === TaskOutput ===

    async fn write_task_output(&self, output: TaskOutput) -> Result<(), DataSpaceError> {
        let arc = Arc::new(output);
        // T-820-07: dedup por `(task_id, seq_num)` + cap por chave — espelha
        // `cache::push_output`. Amostra rejeitada pelo cap não é entregue
        // (a stream real só entrega aceita).
        if accepts_key(&self.outputs, &arc.task_id) {
            let mut entry = self.outputs.entry(arc.task_id.clone()).or_default();
            if let Some(existing) = entry.iter_mut().find(|o| o.seq_num == arc.seq_num) {
                if arc.emitted_at_ns >= existing.emitted_at_ns {
                    *existing = arc.clone();
                }
            } else {
                push_capped(&mut entry, arc.clone());
            }
            let _ = self.output_tx.send(arc);
        }
        Ok(())
    }

    async fn read_task_outputs(
        &self,
        task_id: &str,
    ) -> Result<Vec<Arc<TaskOutput>>, DataSpaceError> {
        Ok(self
            .outputs
            .get(task_id)
            .map(|o| o.clone())
            .unwrap_or_default())
    }

    impl_subscribe!(subscribe_task_outputs, output_tx, Arc<TaskOutput>);

    async fn write_system_metric(&self, metric: SystemMetric) -> Result<(), DataSpaceError> {
        // T-820-07: cap espelhado do cache real.
        let key = (metric.metric_name.clone(), metric.component_id.clone());
        if self.system_metrics.contains_key(&key)
            || self.system_metrics.len() < cache::MAX_TASKS_IN_CACHE
        {
            self.system_metrics.insert(key, metric.clone());
            let _ = self.system_metric_tx.send(metric);
        }
        Ok(())
    }

    async fn read_system_metric(
        &self,
        metric_name: &str,
        component_id: &str,
    ) -> Result<Option<SystemMetric>, DataSpaceError> {
        Ok(self
            .system_metrics
            .get(&(metric_name.to_owned(), component_id.to_owned()))
            .map(|metric| metric.clone()))
    }

    impl_subscribe!(subscribe_system_metrics, system_metric_tx, SystemMetric);

    async fn write_server_status(&self, status: ServerStatus) -> Result<(), DataSpaceError> {
        if accepts_key(&self.server_status, &status.server_id) {
            self.server_status
                .insert(status.server_id.clone(), status.clone());
            let _ = self.server_status_tx.send(status);
        }
        Ok(())
    }

    async fn read_server_status(
        &self,
        server_id: &str,
    ) -> Result<Option<ServerStatus>, DataSpaceError> {
        Ok(self
            .server_status
            .get(server_id)
            .map(|status| status.clone()))
    }

    impl_subscribe!(subscribe_server_status, server_status_tx, ServerStatus);

    // === LLM ===

    async fn write_llm_request(&self, req: LLMInferenceRequest) -> Result<(), DataSpaceError> {
        if accepts_key(&self.llm_requests, &req.request_id) {
            self.llm_requests
                .insert(req.request_id.clone(), req.clone());
            let _ = self.llm_request_tx.send(req);
        }
        Ok(())
    }

    async fn write_llm_result(&self, result: LLMInferenceResult) -> Result<(), DataSpaceError> {
        if accepts_key(&self.llm_results, &result.request_id) {
            let mut entry = self
                .llm_results
                .entry(result.request_id.clone())
                .or_default();
            push_capped(&mut entry, result.clone());
            let _ = self.llm_result_tx.send(result);
        }
        Ok(())
    }

    async fn write_llm_error(&self, error: LLMInferenceError) -> Result<(), DataSpaceError> {
        if accepts_key(&self.llm_errors, &error.request_id) {
            self.llm_errors
                .insert(error.request_id.clone(), error.clone());
            let _ = self.llm_error_tx.send(error);
        }
        Ok(())
    }

    impl_subscribe!(subscribe_llm_requests, llm_request_tx, LLMInferenceRequest);
    impl_subscribe!(subscribe_llm_results, llm_result_tx, LLMInferenceResult);
    impl_subscribe!(subscribe_llm_errors, llm_error_tx, LLMInferenceError);

    // === Context ===

    async fn write_context_snapshot(&self, snap: ContextSnapshot) -> Result<(), DataSpaceError> {
        if accepts_key(&self.context_snapshots, &snap.context_id) {
            self.context_snapshots
                .insert(snap.context_id.clone(), snap.clone());
            let _ = self.context_snapshot_tx.send(snap);
        }
        Ok(())
    }

    async fn write_context_update(&self, update: ContextUpdate) -> Result<(), DataSpaceError> {
        if accepts_key(&self.context_updates, &update.context_id) {
            let mut entry = self
                .context_updates
                .entry(update.context_id.clone())
                .or_default();
            push_capped(&mut entry, update.clone());
            let _ = self.context_update_tx.send(update);
        }
        Ok(())
    }

    impl_subscribe!(
        subscribe_context_snapshots,
        context_snapshot_tx,
        ContextSnapshot
    );
    impl_subscribe!(subscribe_context_updates, context_update_tx, ContextUpdate);

    impl_subscribe!(subscribe_server_statuses, server_status_tx, ServerStatus);

    // === ToolCall ===

    async fn write_tool_call(&self, call: ToolCallRequest) -> Result<(), DataSpaceError> {
        if accepts_key(&self.tool_calls, &call.call_id) {
            self.tool_calls.insert(call.call_id.clone(), call.clone());
            let _ = self.tool_call_tx.send(call);
        }
        Ok(())
    }

    async fn read_tool_call(
        &self,
        call_id: &str,
    ) -> Result<Option<ToolCallRequest>, DataSpaceError> {
        Ok(self.tool_calls.get(call_id).map(|c| c.clone()))
    }

    impl_subscribe!(subscribe_tool_calls, tool_call_tx, ToolCallRequest);

    // === ExecutionTrace ===

    async fn write_execution_trace(
        &self,
        event: ExecutionTraceEvent,
    ) -> Result<(), DataSpaceError> {
        if accepts_key(&self.execution_traces, &event.trace_id) {
            let mut entry = self
                .execution_traces
                .entry(event.trace_id.clone())
                .or_default();
            push_capped(&mut entry, event.clone());
            let _ = self.execution_trace_tx.send(event);
        }
        Ok(())
    }

    impl_subscribe!(
        subscribe_execution_traces,
        execution_trace_tx,
        ExecutionTraceEvent
    );

    // === Security ===

    async fn write_security_snapshot(
        &self,
        snap: SecurityPolicySnapshot,
    ) -> Result<(), DataSpaceError> {
        if accepts_key(&self.security_snapshots, &snap.policy_id) {
            self.security_snapshots
                .insert(snap.policy_id.clone(), snap.clone());
            let _ = self.security_snapshot_tx.send(snap);
        }
        Ok(())
    }

    async fn write_security_update(
        &self,
        update: SecurityPolicyUpdate,
    ) -> Result<(), DataSpaceError> {
        if accepts_key(&self.security_updates, &update.policy_id) {
            let mut entry = self
                .security_updates
                .entry(update.policy_id.clone())
                .or_default();
            push_capped(&mut entry, update.clone());
            let _ = self.security_update_tx.send(update);
        }
        Ok(())
    }

    impl_subscribe!(
        subscribe_security_snapshots,
        security_snapshot_tx,
        SecurityPolicySnapshot
    );
    impl_subscribe!(
        subscribe_security_updates,
        security_update_tx,
        SecurityPolicyUpdate
    );

    // === QoS ===

    async fn write_qos_routing(&self, profile: QoSRoutingProfile) -> Result<(), DataSpaceError> {
        if accepts_key(&self.qos_routing, &profile.profile_id) {
            self.qos_routing
                .insert(profile.profile_id.clone(), profile.clone());
            let _ = self.qos_routing_tx.send(profile);
        }
        Ok(())
    }

    async fn write_qos_metric(&self, metric: QoSMetric) -> Result<(), DataSpaceError> {
        if accepts_key(&self.qos_metrics, &metric.metric_id) {
            self.qos_metrics
                .insert(metric.metric_id.clone(), metric.clone());
            let _ = self.qos_metric_tx.send(metric);
        }
        Ok(())
    }

    async fn write_qos_violation(&self, violation: QoSViolation) -> Result<(), DataSpaceError> {
        if accepts_key(&self.qos_violations, &violation.violation_id) {
            self.qos_violations
                .insert(violation.violation_id.clone(), violation.clone());
            let _ = self.qos_violation_tx.send(violation);
        }
        Ok(())
    }

    async fn write_discovery_event(&self, event: DiscoveryEvent) -> Result<(), DataSpaceError> {
        if accepts_key(&self.discovery_events, &event.event_id) {
            self.discovery_events
                .insert(event.event_id.clone(), event.clone());
            let _ = self.discovery_event_tx.send(event);
        }
        Ok(())
    }

    impl_subscribe!(subscribe_qos_routing, qos_routing_tx, QoSRoutingProfile);
    impl_subscribe!(subscribe_qos_metrics, qos_metric_tx, QoSMetric);
    impl_subscribe!(subscribe_qos_violations, qos_violation_tx, QoSViolation);
    impl_subscribe!(
        subscribe_discovery_events,
        discovery_event_tx,
        DiscoveryEvent
    );

    // === Lifecycle ===

    /// Limpa os caches; o teardown real (broadcast senders, streams) acontece
    /// no `Drop` — quando o mock é dropado, os canais fecham e as streams
    /// terminam (T-820-05/P1-7).
    async fn shutdown(&self) -> Result<(), DataSpaceError> {
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
        Ok(())
    }
}

// Prova em tempo de compilação da ordem dos strengths que o DDS real
// arbitra e o mock ignora (last-write-wins).
const _: () = {
    assert!(
        crate::DataSpace::STRENGTH_CLIENT < crate::DataSpace::STRENGTH_AGENT
            && crate::DataSpace::STRENGTH_AGENT < crate::DataSpace::STRENGTH_ORCHESTRATOR
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    fn task_with_status(id: &str, status: i32) -> Task {
        Task {
            task_id: id.into(),
            status,
            ..Task::default()
        }
    }

    fn output_with_seq(task_id: &str, seq_num: u32, emitted_at_ns: u64) -> TaskOutput {
        TaskOutput {
            task_id: task_id.into(),
            seq_num,
            emitted_at_ns,
            ..TaskOutput::default()
        }
    }

    #[tokio::test]
    async fn write_task_is_last_write_wins_without_ownership_arbitration() {
        // Given: duas escritas sequenciais da mesma task (sem strength por
        // escrita — ver doc de `write_task`)
        let ds = InMemoryDataSpace::new();
        ds.write_task(task_with_status("t", 1)).await.unwrap();
        // When: segunda escrita / Then: última vence (DDS real arbitraría por strength)
        ds.write_task(task_with_status("t", 2)).await.unwrap();
        let back = ds.read_task("t").await.unwrap().expect("task presente");
        assert_eq!(back.status, 2);

        // O contrato que o DDS real impõe e o mock NÃO: cliente<agente<orq
        // (prova em tempo de compilação no const acima do módulo de testes).
    }

    /// T-820-07 (a): o mock agora espelha o filtro de regressão do cache
    /// real (`cache::supersedes`) — status para trás com mesma geração é
    /// rejeitado; `retry_count` maior vence sempre.
    #[tokio::test]
    async fn write_task_rejeita_regressao_de_status() {
        let ds = InMemoryDataSpace::new();
        ds.write_task(task_with_status("r", 2)).await.unwrap();
        // Regressão 2 -> 1 (mesma geração) é rejeitada.
        ds.write_task(task_with_status("r", 1)).await.unwrap();
        let back = ds.read_task("r").await.unwrap().expect("task presente");
        assert_eq!(back.status, 2, "regressão não pode sobrescrever");

        // retry_count maior vence sempre (mesma regra do cache real).
        let mut retry = task_with_status("r", 1);
        retry.retry_count = 1;
        ds.write_task(retry).await.unwrap();
        let back = ds.read_task("r").await.unwrap().expect("task presente");
        assert_eq!(back.status, 1, "retry vence o guard de regressão");
        assert_eq!(back.retry_count, 1);
    }

    /// T-820-07 (b): cap de tasks no mock = mesma constante do cache real
    /// (2048), com eviction de terminais sob pressão.
    #[tokio::test]
    async fn write_task_respeita_cap_com_eviction_de_terminais() {
        let ds = InMemoryDataSpace::new();
        for i in 0..cache::MAX_TASKS_IN_CACHE {
            ds.write_task(task_with_status(&format!("fill-{i}"), 0))
                .await
                .unwrap();
        }
        // Cap atingido: task nova não-terminal é rejeitada (não entra, não
        // estoura o limite).
        ds.write_task(task_with_status("overflow", 0))
            .await
            .unwrap();
        assert_eq!(ds.tasks.len(), cache::MAX_TASKS_IN_CACHE);
        assert!(ds.read_task("overflow").await.unwrap().is_none());

        // Eviction: uma task terminal antiga sai; o cache volta a aceitar.
        let mut terminal = task_with_status("fill-0", 4);
        terminal.completed_at_ns = 1; // "completada" no passado distante
        ds.write_task(terminal).await.unwrap();
        assert_eq!(
            ds.read_task("fill-0")
                .await
                .unwrap()
                .expect("terminal aceita")
                .status,
            4
        );
        ds.write_task(task_with_status("pos-eviction", 0))
            .await
            .unwrap();
        assert!(ds.read_task("pos-eviction").await.unwrap().is_some());
    }

    /// T-820-07 (c): dedup de outputs por `(task_id, seq_num)` e cap de
    /// chunks por chave, espelhando `cache::push_output`.
    #[tokio::test]
    async fn write_task_output_dedup_e_cap() {
        let ds = InMemoryDataSpace::new();
        ds.write_task_output(output_with_seq("o", 0, 10))
            .await
            .unwrap();
        ds.write_task_output(output_with_seq("o", 1, 20))
            .await
            .unwrap();
        // Reentrega do seq 0 com timestamp maior substitui; com menor, ignora.
        ds.write_task_output(output_with_seq("o", 0, 30))
            .await
            .unwrap();
        ds.write_task_output(output_with_seq("o", 0, 5))
            .await
            .unwrap();
        let outs = ds.read_task_outputs("o").await.unwrap();
        assert_eq!(outs.len(), 2, "reentrega não duplica");
        assert_eq!(
            outs.iter()
                .find(|o| o.seq_num == 0)
                .expect("seq 0")
                .emitted_at_ns,
            30
        );

        // Cap de chunks por chave (256): o mais antigo sai, o total não passa.
        for seq in 0..(cache::MAX_CHUNKS_PER_KEY as u32 + 10) {
            ds.write_task_output(output_with_seq("cap", seq, seq as u64))
                .await
                .unwrap();
        }
        let capped = ds.read_task_outputs("cap").await.unwrap();
        assert_eq!(capped.len(), cache::MAX_CHUNKS_PER_KEY);
        assert_eq!(capped[0].seq_num, 10, "o mais antigo é descartado");
    }
}
