//! Observação DDS no Studio (P7/P9): leitura viva do domínio — agentes,
//! tool calls e métricas de sistema — sem publicar nada (só `ownership 0`).
//!
//! Atrás da feature `dds`: sem ela, o Studio continua HTTP-only. Cada coleta
//! abre um `DataSpace` efêmero, drena o stream pela janela e fecha — sem
//! thread permanente e sem roubar amostras (`read`, nunca `take`).

use std::collections::HashMap;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::{AgentState, SystemMetric, ToolCallRequest};
use dds_dataspace::DataSpace;
use futures::StreamExt;
use thiserror::Error;

/// Linha de topologia exibida na GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRow {
    pub agent_id: String,
    pub model: String,
    pub slots_busy: u32,
    pub slots_total: u32,
}

/// Linha de tool call exibida na GUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub call_id: String,
    pub tool_name: String,
    pub status: i32,
}

/// Métrica de sistema exibida na GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricRow {
    pub source: String,
    pub name: String,
    pub value: f64,
}

/// Erros da observação DDS.
#[derive(Debug, Error)]
pub enum ObserveError {
    /// Domínio inacessível (DDS fora, participante falhou, runtime falhou).
    #[error("falha ao observar o dominio {domain}: {detail}")]
    Unavailable { domain: u32, detail: String },
}

fn runtime(domain: u32) -> Result<tokio::runtime::Runtime, ObserveError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| ObserveError::Unavailable {
            domain,
            detail: err.to_string(),
        })
}

fn dataspace(domain: u32) -> Result<DataSpace, ObserveError> {
    DataSpace::new(domain, 0).map_err(|err| ObserveError::Unavailable {
        domain,
        detail: err.to_string(),
    })
}

async fn drain<S, K, V>(stream: S, window: Duration, key: impl Fn(&V) -> K) -> HashMap<K, V>
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

/// Foto do domínio: agentes, tool calls e métricas drenados na mesma janela.
#[derive(Debug, Clone, Default)]
pub struct DdsSnapshot {
    pub agents: Vec<AgentRow>,
    pub tools: Vec<ToolRow>,
    pub metrics: Vec<MetricRow>,
}

fn sorted<K: Ord, V>(rows: HashMap<K, V>) -> Vec<V> {
    let mut rows: Vec<(K, V)> = rows.into_iter().collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, value)| value).collect()
}

/// Observa o domínio uma janela: um `DataSpace`, três drenos concorrentes.
///
/// `DataSpace::new` exige contexto Tokio (WaitSet compartilhado) — por isso
/// nasce dentro do `block_on`, nunca fora dele.
pub fn observe(domain: u32, window: Duration) -> Result<DdsSnapshot, ObserveError> {
    let rt = runtime(domain)?;
    rt.block_on(async {
        let space = dataspace(domain)?;
        let (agents, tools, metrics) = tokio::join!(
            drain(space.stream_agent_states(), window, |state: &AgentState| {
                state.agent_id.clone()
            }),
            drain(
                space.stream_tool_calls(),
                window,
                |call: &ToolCallRequest| call.call_id.clone()
            ),
            drain(
                space.stream_system_metrics(),
                window,
                |metric: &SystemMetric| format!("{}:{}", metric.component_id, metric.metric_name)
            ),
        );
        Ok(DdsSnapshot {
            agents: sorted(agents).iter().map(agent_row).collect(),
            tools: sorted(tools).iter().map(tool_row).collect(),
            metrics: sorted(metrics).iter().map(metric_row).collect(),
        })
    })
}

/// Estado do painel de topologia DDS.
#[derive(Debug, Clone)]
pub struct DdsState {
    pub domain: u32,
    pub window_secs: u64,
    pub snapshot: DdsSnapshot,
    pub error: String,
}

impl DdsState {
    /// Padrão honesto: domínio vivo local, janela de 3s para heartbeats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            domain: 42,
            window_secs: 3,
            snapshot: DdsSnapshot::default(),
            error: String::new(),
        }
    }

    /// Observa o domínio; erro preserva a foto anterior e registra o motivo.
    pub fn refresh(&mut self) {
        match observe(self.domain, Duration::from_secs(self.window_secs.max(1))) {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.error.clear();
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }
}

impl Default for DdsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_row_maps_contract_fields() {
        let state = AgentState {
            agent_id: String::from("a-1"),
            model: String::from("m"),
            slots_busy: 2,
            slots_total: 8,
            ..AgentState::default()
        };

        assert_eq!(
            agent_row(&state),
            AgentRow {
                agent_id: String::from("a-1"),
                model: String::from("m"),
                slots_busy: 2,
                slots_total: 8,
            }
        );
    }

    #[test]
    fn tool_row_maps_contract_fields() {
        let call = ToolCallRequest {
            call_id: String::from("c-1"),
            tool_name: String::from("ler_arquivo"),
            status: 3,
            ..ToolCallRequest::default()
        };

        assert_eq!(
            tool_row(&call),
            ToolRow {
                call_id: String::from("c-1"),
                tool_name: String::from("ler_arquivo"),
                status: 3,
            }
        );
    }

    #[test]
    fn metric_row_formats_name_with_unit() {
        let metric = SystemMetric {
            component_id: String::from("no-a"),
            metric_name: String::from("cpu"),
            value: 0.5,
            unit: String::from("fração"),
            ..SystemMetric::default()
        };

        assert_eq!(
            metric_row(&metric),
            MetricRow {
                source: String::from("no-a"),
                name: String::from("cpu (fração)"),
                value: 0.5,
            }
        );
    }
}
