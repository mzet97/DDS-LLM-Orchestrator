//! DataSpaceApi trait — interface abstrata para o DDS Data Space (REQ-301, REQ-309).
//!
//! Esta trait permite que a lógica de negócio (agent, orchestrator) seja testada
//! com um mock InMemoryDataSpace sem depender do CycloneDDS real.

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

/// Erros do DataSpace.
#[derive(Debug, thiserror::Error)]
pub enum DataSpaceError {
    #[error("DDS error: {0}")]
    Dds(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("write failed: {0}")]
    WriteFailed(String),
}

/// Interface abstrata para o DDS Data Space.
#[async_trait::async_trait]
pub trait DataSpaceApi: Send + Sync {
    // === Tasks ===

    /// Publica ou atualiza uma task.
    async fn write_task(&self, task: Task) -> Result<(), DataSpaceError>;

    /// Publica a Task via writer de strength do CLIENTE (10), sem assumir o
    /// ownership da instância — usado pelo reaper/reatribuição para não
    /// congelar a task (T-820-03; Exclusive Ownership: writers de strength
    /// maior tornam-se donos e impedem claims futuros).
    ///
    /// ATENÇÃO: sob Exclusive Ownership, um write de strength MENOR que a do
    /// dono atual é descartado pelo RHC (invisível aos leitores) enquanto o
    /// writer do dono seguir registrado — o ownership só é liberado pela
    /// DESTRUIÇÃO desse writer (`relinquish_ownership` no RHC do CycloneDDS;
    /// morte real do processo → lease do DDSI; delete gracioso → imediato).
    /// Um dispose também passa pelo filtro e AINDA transfere o ownership para
    /// o writer que o fez (`update_inst_have_wr_iid` no dispose aceito), então
    /// NÃO é caminho para devolver a instância a strength menor. Consumidores
    /// que reatribuem (reaper) devem re-publicar periodicamente até o mesh
    /// refletir o PENDING — ver o republisher do orquestrador (T-820-03).
    async fn write_task_without_ownership(&self, task: Task) -> Result<(), DataSpaceError>;

    /// Lê uma task por ID. Retorna `Arc<Task>` — o cache já guarda `Arc`
    /// internamente (Fase 3 do `OPTIMIZATION_PLAN.md`); antes este método
    /// desreferenciava e clonava a `Task` inteira em cada leitura.
    async fn read_task(&self, task_id: &str) -> Result<Option<Arc<Task>>, DataSpaceError>;

    /// Lista todas as tasks (mesma nota de `Arc` acima).
    async fn all_tasks(&self) -> Result<Vec<Arc<Task>>, DataSpaceError>;

    /// Retorna um stream de tasks (wakeup por amostra, sem polling).
    fn subscribe_tasks(&self) -> Pin<Box<dyn Stream<Item = Arc<Task>> + Send>>;

    // === Agents ===

    /// Publica ou atualiza estado do agente.
    async fn write_agent_state(&self, state: AgentState) -> Result<(), DataSpaceError>;

    /// Lê estado do agente por ID.
    async fn read_agent_state(&self, agent_id: &str) -> Result<Option<AgentState>, DataSpaceError>;

    /// Lista todos os agentes.
    async fn all_agents(&self) -> Result<Vec<AgentState>, DataSpaceError>;

    /// Retorna um stream de estados de agentes.
    fn subscribe_agent_states(&self) -> Pin<Box<dyn Stream<Item = AgentState> + Send>>;

    // === TaskOutput ===

    /// Publica um output de task.
    async fn write_task_output(&self, output: TaskOutput) -> Result<(), DataSpaceError>;

    /// Lê outputs de uma task (`Arc` — ver nota em `read_task`).
    async fn read_task_outputs(
        &self,
        task_id: &str,
    ) -> Result<Vec<Arc<TaskOutput>>, DataSpaceError>;

    /// Retorna um stream de outputs.
    fn subscribe_task_outputs(&self) -> Pin<Box<dyn Stream<Item = Arc<TaskOutput>> + Send>>;

    // === Runtime telemetry ===

    /// Publishes one system metric sample (REQ-708).
    async fn write_system_metric(&self, metric: SystemMetric) -> Result<(), DataSpaceError>;

    /// Reads the most recent metric for a `(metric_name, component_id)` key.
    async fn read_system_metric(
        &self,
        metric_name: &str,
        component_id: &str,
    ) -> Result<Option<SystemMetric>, DataSpaceError>;

    /// Streams system metrics as DDS samples arrive.
    fn subscribe_system_metrics(&self) -> Pin<Box<dyn Stream<Item = SystemMetric> + Send>>;

    /// Publishes one llama-server status sample (REQ-708).
    async fn write_server_status(&self, status: ServerStatus) -> Result<(), DataSpaceError>;

    /// Reads the most recent status for a server.
    async fn read_server_status(
        &self,
        server_id: &str,
    ) -> Result<Option<ServerStatus>, DataSpaceError>;

    /// Streams server status samples as they arrive.
    fn subscribe_server_status(&self) -> Pin<Box<dyn Stream<Item = ServerStatus> + Send>>;

    // === LLM ===

    /// Publica um request de inferência LLM.
    async fn write_llm_request(&self, req: LLMInferenceRequest) -> Result<(), DataSpaceError>;

    /// Publica um resultado de inferência LLM.
    async fn write_llm_result(&self, result: LLMInferenceResult) -> Result<(), DataSpaceError>;

    /// Publica um erro de inferência LLM.
    async fn write_llm_error(&self, error: LLMInferenceError) -> Result<(), DataSpaceError>;

    /// Retorna um stream de requests LLM.
    fn subscribe_llm_requests(&self) -> Pin<Box<dyn Stream<Item = LLMInferenceRequest> + Send>>;

    /// Retorna um stream de resultados LLM.
    fn subscribe_llm_results(&self) -> Pin<Box<dyn Stream<Item = LLMInferenceResult> + Send>>;

    /// Retorna um stream de erros LLM.
    fn subscribe_llm_errors(&self) -> Pin<Box<dyn Stream<Item = LLMInferenceError> + Send>>;

    // === Context ===

    /// Publica um snapshot de contexto.
    async fn write_context_snapshot(&self, snap: ContextSnapshot) -> Result<(), DataSpaceError>;

    /// Publica um update de contexto.
    async fn write_context_update(&self, update: ContextUpdate) -> Result<(), DataSpaceError>;

    /// Retorna um stream de snapshots de contexto.
    fn subscribe_context_snapshots(&self) -> Pin<Box<dyn Stream<Item = ContextSnapshot> + Send>>;

    /// Retorna um stream de updates de contexto.
    fn subscribe_context_updates(&self) -> Pin<Box<dyn Stream<Item = ContextUpdate> + Send>>;

    // === Telemetria de sistema (plural preservado: chamadores em
    // contract.rs usam `subscribe_server_statuses`; t808 usa o singular) ===

    /// Retorna um stream de status do servidor (forma plural).
    fn subscribe_server_statuses(&self) -> Pin<Box<dyn Stream<Item = ServerStatus> + Send>>;

    // === ToolCall ===

    /// Publica um request de tool call (writer do papel GATEWAY/agente —
    /// reivindica e evolui a instância).
    async fn write_tool_call(&self, call: ToolCallRequest) -> Result<(), DataSpaceError>;

    /// Publica o pedido SEM ser dono (strength CLIENTE) — orquestrador/sonda
    /// que só põe o `ToolCall.Request` inicial (T-890-06, espelho do protocolo
    /// de Tasks com `Ownership=Exclusive`).
    async fn write_tool_call_without_ownership(
        &self,
        call: ToolCallRequest,
    ) -> Result<(), DataSpaceError> {
        let _ = call;
        Err(DataSpaceError::WriteFailed(
            "write_tool_call_without_ownership não implementado por este backend".to_owned(),
        ))
    }

    /// Lê um tool call por ID. Sem default: `Ok(None)` silencioso escondia
    /// implementador incompleto (M2) — todo backend declara o seu.
    async fn read_tool_call(
        &self,
        call_id: &str,
    ) -> Result<Option<ToolCallRequest>, DataSpaceError>;

    /// Retorna um stream de tool calls.
    fn subscribe_tool_calls(&self) -> Pin<Box<dyn Stream<Item = ToolCallRequest> + Send>>;

    // === ExecutionTrace ===

    /// Publica um evento de trace.
    async fn write_execution_trace(&self, event: ExecutionTraceEvent)
        -> Result<(), DataSpaceError>;

    /// Retorna um stream de eventos de trace.
    fn subscribe_execution_traces(&self)
        -> Pin<Box<dyn Stream<Item = ExecutionTraceEvent> + Send>>;

    // === Security ===

    /// Publica um snapshot de política de segurança.
    async fn write_security_snapshot(
        &self,
        snap: SecurityPolicySnapshot,
    ) -> Result<(), DataSpaceError>;

    /// Publica um update de política de segurança.
    async fn write_security_update(
        &self,
        update: SecurityPolicyUpdate,
    ) -> Result<(), DataSpaceError>;

    /// Retorna um stream de snapshots de segurança.
    fn subscribe_security_snapshots(
        &self,
    ) -> Pin<Box<dyn Stream<Item = SecurityPolicySnapshot> + Send>>;

    /// Retorna um stream de updates de segurança.
    fn subscribe_security_updates(
        &self,
    ) -> Pin<Box<dyn Stream<Item = SecurityPolicyUpdate> + Send>>;

    // === QoS ===

    /// Publica um perfil de roteamento QoS.
    async fn write_qos_routing(&self, profile: QoSRoutingProfile) -> Result<(), DataSpaceError>;

    /// Publica uma métrica QoS.
    async fn write_qos_metric(&self, metric: QoSMetric) -> Result<(), DataSpaceError>;

    /// Publica uma violação QoS.
    async fn write_qos_violation(&self, violation: QoSViolation) -> Result<(), DataSpaceError>;

    /// Publica um evento de discovery.
    ///
    /// Sem produtor nativo em Rust (M3): hoje os eventos chegam via
    /// `qos_monitor` Python ou escrita direta (testes). O lado Rust
    /// consome via `subscribe_discovery_events` → `qos_collector`.
    async fn write_discovery_event(&self, event: DiscoveryEvent) -> Result<(), DataSpaceError>;

    /// Retorna um stream de perfis de roteamento QoS.
    fn subscribe_qos_routing(&self) -> Pin<Box<dyn Stream<Item = QoSRoutingProfile> + Send>>;

    /// Retorna um stream de métricas QoS.
    fn subscribe_qos_metrics(&self) -> Pin<Box<dyn Stream<Item = QoSMetric> + Send>>;

    /// Retorna um stream de violações QoS.
    fn subscribe_qos_violations(&self) -> Pin<Box<dyn Stream<Item = QoSViolation> + Send>>;

    /// Retorna um stream de eventos de discovery.
    fn subscribe_discovery_events(&self) -> Pin<Box<dyn Stream<Item = DiscoveryEvent> + Send>>;

    // === Lifecycle ===

    /// Limpa os caches; o teardown real (participant/waitset/streams)
    /// acontece no `Drop` — o `Drop` do `SharedWaitSet` marca a flag de
    /// shutdown e acorda os registros, e cada `stream_*` termina
    /// graciosamente (T-820-05/P1-7).
    async fn shutdown(&self) -> Result<(), DataSpaceError>;
}
