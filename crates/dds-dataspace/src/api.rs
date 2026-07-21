//! DataSpaceApi trait — interface abstrata para o DDS Data Space (REQ-301, REQ-309).
//!
//! Esta trait permite que a lógica de negócio (agent, orchestrator) seja testada
//! com um mock InMemoryDataSpace sem depender do CycloneDDS real.

use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, ContextSnapshot, ContextUpdate, DiscoveryEvent, ExecutionTraceEvent, QoSMetric,
    QoSRoutingProfile, QoSViolation, SecurityPolicySnapshot, SecurityPolicyUpdate, Task,
    TaskOutput, ToolCallRequest,
};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
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

    // === ToolCall ===

    /// Publica um request de tool call.
    async fn write_tool_call(&self, call: ToolCallRequest) -> Result<(), DataSpaceError>;

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

    /// Encerra o DataSpace, liberando recursos.
    async fn shutdown(&self) -> Result<(), DataSpaceError>;
}
