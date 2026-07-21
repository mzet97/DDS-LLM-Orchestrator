//! InMemoryDataSpace — mock para testes (REQ-309, T-301).
//!
//! Implementa DataSpaceApi usando estruturas de dados em memória.
//! Usado para contract tests A/B (mesma bateria roda contra mock e DDS real).

use crate::api::{DataSpaceApi, DataSpaceError};
use async_stream::stream;
use dashmap::DashMap;
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
use tokio::sync::broadcast;

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

    async fn write_task(&self, task: Task) -> Result<(), DataSpaceError> {
        let arc = Arc::new(task);
        self.tasks.insert(arc.task_id.clone(), arc.clone());
        let _ = self.task_tx.send(arc);
        Ok(())
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
        self.agents.insert(state.agent_id.clone(), state.clone());
        let _ = self.agent_tx.send(state);
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
        self.outputs
            .entry(arc.task_id.clone())
            .or_default()
            .push(arc.clone());
        let _ = self.output_tx.send(arc);
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

    // === LLM ===

    async fn write_llm_request(&self, req: LLMInferenceRequest) -> Result<(), DataSpaceError> {
        self.llm_requests
            .insert(req.request_id.clone(), req.clone());
        let _ = self.llm_request_tx.send(req);
        Ok(())
    }

    async fn write_llm_result(&self, result: LLMInferenceResult) -> Result<(), DataSpaceError> {
        self.llm_results
            .entry(result.request_id.clone())
            .or_default()
            .push(result.clone());
        let _ = self.llm_result_tx.send(result);
        Ok(())
    }

    async fn write_llm_error(&self, error: LLMInferenceError) -> Result<(), DataSpaceError> {
        self.llm_errors
            .insert(error.request_id.clone(), error.clone());
        let _ = self.llm_error_tx.send(error);
        Ok(())
    }

    impl_subscribe!(subscribe_llm_requests, llm_request_tx, LLMInferenceRequest);
    impl_subscribe!(subscribe_llm_results, llm_result_tx, LLMInferenceResult);
    impl_subscribe!(subscribe_llm_errors, llm_error_tx, LLMInferenceError);

    // === Context ===

    async fn write_context_snapshot(&self, snap: ContextSnapshot) -> Result<(), DataSpaceError> {
        self.context_snapshots
            .insert(snap.context_id.clone(), snap.clone());
        let _ = self.context_snapshot_tx.send(snap);
        Ok(())
    }

    async fn write_context_update(&self, update: ContextUpdate) -> Result<(), DataSpaceError> {
        self.context_updates
            .entry(update.context_id.clone())
            .or_default()
            .push(update.clone());
        let _ = self.context_update_tx.send(update);
        Ok(())
    }

    impl_subscribe!(
        subscribe_context_snapshots,
        context_snapshot_tx,
        ContextSnapshot
    );
    impl_subscribe!(subscribe_context_updates, context_update_tx, ContextUpdate);

    // === ToolCall ===

    async fn write_tool_call(&self, call: ToolCallRequest) -> Result<(), DataSpaceError> {
        self.tool_calls.insert(call.call_id.clone(), call.clone());
        let _ = self.tool_call_tx.send(call);
        Ok(())
    }

    impl_subscribe!(subscribe_tool_calls, tool_call_tx, ToolCallRequest);

    // === ExecutionTrace ===

    async fn write_execution_trace(
        &self,
        event: ExecutionTraceEvent,
    ) -> Result<(), DataSpaceError> {
        self.execution_traces
            .entry(event.trace_id.clone())
            .or_default()
            .push(event.clone());
        let _ = self.execution_trace_tx.send(event);
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
        self.security_snapshots
            .insert(snap.policy_id.clone(), snap.clone());
        let _ = self.security_snapshot_tx.send(snap);
        Ok(())
    }

    async fn write_security_update(
        &self,
        update: SecurityPolicyUpdate,
    ) -> Result<(), DataSpaceError> {
        self.security_updates
            .entry(update.policy_id.clone())
            .or_default()
            .push(update.clone());
        let _ = self.security_update_tx.send(update);
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
        self.qos_routing
            .insert(profile.profile_id.clone(), profile.clone());
        let _ = self.qos_routing_tx.send(profile);
        Ok(())
    }

    async fn write_qos_metric(&self, metric: QoSMetric) -> Result<(), DataSpaceError> {
        self.qos_metrics
            .insert(metric.metric_id.clone(), metric.clone());
        let _ = self.qos_metric_tx.send(metric);
        Ok(())
    }

    async fn write_qos_violation(&self, violation: QoSViolation) -> Result<(), DataSpaceError> {
        self.qos_violations
            .insert(violation.violation_id.clone(), violation.clone());
        let _ = self.qos_violation_tx.send(violation);
        Ok(())
    }

    async fn write_discovery_event(&self, event: DiscoveryEvent) -> Result<(), DataSpaceError> {
        self.discovery_events
            .insert(event.event_id.clone(), event.clone());
        let _ = self.discovery_event_tx.send(event);
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

    async fn shutdown(&self) -> Result<(), DataSpaceError> {
        self.tasks.clear();
        self.agents.clear();
        self.outputs.clear();
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
