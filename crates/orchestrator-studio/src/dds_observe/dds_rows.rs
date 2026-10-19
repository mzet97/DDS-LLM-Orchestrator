//! Parte de `dds_observe` (extraída para manter os arquivos revisáveis).

use super::*;

pub(crate) fn runtime(domain: u32) -> Result<tokio::runtime::Runtime, ObserveError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| ObserveError::Unavailable {
            domain,
            detail: err.to_string(),
        })
}

pub(crate) fn dataspace(domain: u32) -> Result<DataSpace, ObserveError> {
    DataSpace::new(domain, 0).map_err(|err| ObserveError::Unavailable {
        domain,
        detail: err.to_string(),
    })
}

pub(crate) async fn drain<S, K, V>(
    stream: S,
    window: Duration,
    key: impl Fn(&V) -> K,
) -> HashMap<K, V>
where
    S: futures::Stream<Item = std::sync::Arc<V>>,
    K: Eq + std::hash::Hash,
    V: Clone,
{
    let mut stream = Box::pin(stream);
    let mut seen = HashMap::new();
    let deadline = tokio::time::sleep(window);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            () = &mut deadline => break,
            item = stream.next() => match item {
                Some(arc) => {
                    let value = (*arc).clone();
                    seen.insert(key(&value), value);
                }
                None => break,
            },
        }
    }
    seen
}

/// Mapeamento puro: `AgentState` do fio → linha da GUI.
#[must_use]
pub fn agent_row(state: &AgentState) -> AgentRow {
    AgentRow {
        agent_id: state.agent_id.clone(),
        model: state.model.clone(),
        slots_busy: state.slots_busy,
        slots_total: state.slots_total,
    }
}

/// Mapeamento puro: `ToolCallRequest` do fio → linha da GUI.
#[must_use]
pub fn tool_row(call: &ToolCallRequest) -> ToolRow {
    ToolRow {
        call_id: call.call_id.clone(),
        tool_name: call.tool_name.clone(),
        status: call.status,
        requester_id: call.requester_id.clone(),
        security_level: call.security_level,
        result_preview: preview(&call.result_json, 80),
        arguments_json: capped(&call.arguments_json),
        result_json: capped(&call.result_json),
        duration_ms: if call.completed_at_ns > call.created_at_ns {
            (call.completed_at_ns - call.created_at_ns) / 1_000_000
        } else {
            0
        },
    }
}

/// Mapeamento puro: `Task` do fio → linha da GUI (aba TAREFAS DDS).
#[must_use]
pub fn task_row(task: &Task) -> TaskRow {
    TaskRow {
        task_id: task.task_id.clone(),
        client_id: task.client_id.clone(),
        assigned_agent: task.assigned_agent.clone(),
        model_name: task.model_name.clone(),
        status: task.status,
        priority: task.priority,
        retry_count: task.retry_count,
        created_at_ns: task.created_at_ns,
        completed_at_ns: task.completed_at_ns,
    }
}

/// Mapeamento puro: `TaskOutput` do fio → linha da GUI (log de tarefas).
#[must_use]
pub fn task_output_row(output: &TaskOutput) -> TaskOutputRow {
    TaskOutputRow {
        task_id: output.task_id.clone(),
        seq_num: output.seq_num,
        agent_id: output.agent_id.clone(),
        is_final: output.is_final,
        token_count: output.token_count,
        emitted_at_ns: output.emitted_at_ns,
        content: capped(&output.content),
    }
}

/// Rótulo do ciclo de vida da tarefa (`orch-common::TaskStatus` canônico:
/// PENDING=0 → ASSIGNED=1 → RUNNING=2 → DONE=3 / FAILED=4).
#[must_use]
pub fn task_status_label(status: i32) -> &'static str {
    match status {
        0 => "PENDING",
        1 => "ASSIGNED",
        2 => "RUNNING",
        3 => "DONE",
        4 => "FAILED",
        _ => "desconhecido",
    }
}

/// Trunca para exibição preservando caracteres (não bytes) e achatando linha.
#[must_use]
fn preview(text: &str, max_chars: usize) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    if flat.chars().count() <= max_chars {
        flat
    } else {
        let cut: String = flat.chars().take(max_chars).collect();
        format!("{cut}…")
    }
}

/// Mapeamento puro: `SystemMetric` do fio → linha da GUI.
#[must_use]
pub fn metric_row(metric: &SystemMetric) -> MetricRow {
    MetricRow {
        source: metric.component_id.clone(),
        name: format!("{} ({})", metric.metric_name, metric.unit),
        value: f64::from(metric.value),
    }
}

/// Mapeamento puro: `DiscoveryEvent` do fio → linha da GUI.
#[must_use]
pub fn discovery_row(event: &DiscoveryEvent) -> DiscoveryRow {
    DiscoveryRow {
        event_type: event.event_type.clone(),
        topic_name: event.topic_name.clone(),
        remote_entity: event.remote_entity.clone(),
    }
}

/// Mapeamento puro: `StudioNodePresence` do fio → linha da GUI (T-890).
#[must_use]
pub fn studio_node_row(presence: &StudioNodePresence) -> StudioNodeRow {
    StudioNodeRow {
        node_id: presence.node_id.clone(),
        url: presence.url.clone(),
        token_required: presence.token_required,
    }
}

/// Rótulo do status canônico do contrato (`orch-common::ToolCallStatus`:
/// PENDING=0, ALLOWED=1, DENIED=2, EXECUTING=3, COMPLETED=4, FAILED=5).
#[must_use]
pub fn status_label(status: i32) -> &'static str {
    match status {
        0 => "PENDING",
        1 => "ALLOWED",
        2 => "DENIED",
        3 => "EXECUTING",
        4 => "COMPLETED",
        5 => "FAILED",
        _ => "desconhecido",
    }
}

/// Rótulo do nível de segurança — vocabulário do PRD v1.0 (tela 3.13),
/// que sobrescreve o nome do enum no IDL (`SecurityLevel`): N0 leitura,
/// N1 execução isolada, N2 mutação no host.
#[must_use]
pub fn security_level_label(level: i32) -> String {
    match level {
        0 => String::from("N0 READ_ONLY"),
        1 => String::from("N1 SANDBOX_EXEC"),
        2 => String::from("N2 HOST_MUTATION"),
        other => format!("N{other} ?"),
    }
}

/// Foto do domínio: tarefas, saídas, agentes, tool calls, métricas,
/// descoberta e nós Studio drenados na mesma janela.
#[derive(Debug, Clone, Default)]
pub struct DdsSnapshot {
    pub tasks: Vec<TaskRow>,
    pub task_outputs: Vec<TaskOutputRow>,
    pub agents: Vec<AgentRow>,
    pub tools: Vec<ToolRow>,
    pub metrics: Vec<MetricRow>,
    pub discoveries: Vec<DiscoveryRow>,
    pub studio_nodes: Vec<StudioNodeRow>,
}
