//! Observação DDS no Studio (P7/P9): leitura viva do domínio — agentes,
//! tool calls, métricas de sistema e presença de nós Studio (`Studio.
//! NodePresence`, T-890) — sem publicar nada (só `ownership 0`).
//!
//! Atrás da feature `dds`: sem ela, o Studio continua HTTP-only. Cada coleta
//! abre um `DataSpace` efêmero, drena o stream pela janela e fecha — sem
//! thread permanente e sem roubar amostras (`read`, nunca `take`).
//!
//! A coleta (`observe`, `block_on` de 1–30 s) roda em THREAD de trabalho
//! (padrão `models.rs`: thread + mpsc + `poll` por frame) — a thread de UI
//! nunca bloqueia (REQ/T-820-19).

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, DiscoveryEvent, StudioNodePresence, SystemMetric, ToolCallRequest,
};
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
    /// T-890-06: quem pediu a ferramenta (governança por requester).
    pub requester_id: String,
    /// Nível de segurança pedido (o policy-engine decide por ele).
    pub security_level: i32,
    /// Prévia do resultado truncada (a íntegra fica no fio).
    pub result_preview: String,
}

/// Métrica de sistema exibida na GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricRow {
    pub source: String,
    pub name: String,
    pub value: f64,
}

/// Evento de descoberta exibido na GUI (P3a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRow {
    pub event_type: String,
    pub topic_name: String,
    pub remote_entity: String,
}

/// Linha de nó Studio exibida na GUI (T-890 — 19º tópico
/// `Studio.NodePresence`; instalação viva no domínio).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioNodeRow {
    pub node_id: String,
    pub url: String,
    pub token_required: bool,
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
        requester_id: call.requester_id.clone(),
        security_level: call.security_level,
        result_preview: preview(&call.result_json, 80),
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

/// Rótulo do nível de segurança (contrato `SecurityLevel`: 0=PUBLIC,
/// 1=INTERNAL — só esses dois confirmados no IDL; demais aparecem crus).
#[must_use]
pub fn security_level_label(level: i32) -> String {
    match level {
        0 => String::from("0 · public"),
        1 => String::from("1 · internal"),
        other => format!("{other} · ?"),
    }
}

/// Foto do domínio: agentes, tool calls, métricas, descoberta e nós Studio
/// drenados na mesma janela.
#[derive(Debug, Clone, Default)]
pub struct DdsSnapshot {
    pub agents: Vec<AgentRow>,
    pub tools: Vec<ToolRow>,
    pub metrics: Vec<MetricRow>,
    pub discoveries: Vec<DiscoveryRow>,
    pub studio_nodes: Vec<StudioNodeRow>,
}

fn sorted<K: Ord, V>(rows: HashMap<K, V>) -> Vec<V> {
    let mut rows: Vec<(K, V)> = rows.into_iter().collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, value)| value).collect()
}

/// Observa o domínio uma janela: um `DataSpace`, cinco drenos concorrentes.
///
/// `DataSpace::new` exige contexto Tokio (WaitSet compartilhado) — por isso
/// nasce dentro do `block_on`, nunca fora dele.
pub fn observe(domain: u32, window: Duration) -> Result<DdsSnapshot, ObserveError> {
    let rt = runtime(domain)?;
    rt.block_on(async {
        let space = dataspace(domain)?;
        let (agents, tools, metrics, discoveries, studio_nodes) = tokio::join!(
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
            drain(
                space.stream_discovery_events(),
                window,
                |event: &DiscoveryEvent| event.event_id.clone()
            ),
            // T-890: presença das instalações do Studio (19º tópico).
            drain(
                space.stream_studio_node_presences(),
                window,
                |presence: &StudioNodePresence| presence.node_id.clone()
            ),
        );
        Ok(DdsSnapshot {
            agents: sorted(agents).iter().map(agent_row).collect(),
            tools: sorted(tools).iter().map(tool_row).collect(),
            metrics: sorted(metrics).iter().map(metric_row).collect(),
            discoveries: sorted(discoveries).iter().map(discovery_row).collect(),
            studio_nodes: sorted(studio_nodes).iter().map(studio_node_row).collect(),
        })
    })
}

/// Mensagem do worker de observação.
struct ObserveMsg(Result<DdsSnapshot, String>);

/// Estado do painel de topologia DDS.
#[derive(Debug)]
pub struct DdsState {
    pub domain: u32,
    pub window_secs: u64,
    pub snapshot: DdsSnapshot,
    pub error: String,
    /// `true` enquanto a coleta roda em background (`poll` drena).
    pub busy: bool,
    /// Aba ativa do painel de coleções (0=agentes, 1=tools, 2=métricas,
    /// 3=descoberta, 4=instalações) — estado de apresentação da view.
    pub tab: u8,
    /// Filtro por substring aplicado às tabelas de coleções.
    pub filter: String,
    receiver: Option<mpsc::Receiver<ObserveMsg>>,
}

impl DdsState {
    /// Padrão honesto: domínio vivo local, janela de 3s para heartbeats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Domínio default = domínio da descoberta (env STUDIO_DDS_DOMAIN,
            // default 170 = laboratório) — abrir Topologia/Ferramentas já
            // observa o domínio certo (T-890-03).
            domain: std::env::var("STUDIO_DDS_DOMAIN")
                .ok()
                .and_then(|d| d.trim().parse().ok())
                .unwrap_or(170),
            window_secs: 3,
            snapshot: DdsSnapshot::default(),
            error: String::new(),
            busy: false,
            tab: 0,
            filter: String::new(),
            receiver: None,
        }
    }

    /// Observa o domínio em THREAD de trabalho (`observe` faz `block_on` de
    /// 1–30 s — nunca na thread de UI, REQ/T-820-19). Erro preserva a foto
    /// anterior e registra o motivo. Clique durante `busy` é ignorado.
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let domain = self.domain;
        let window = Duration::from_secs(self.window_secs.max(1));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ObserveMsg(
                observe(domain, window).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.error = "observando domínio…".into();
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(ObserveMsg(result)) = rx.try_recv() {
                match result {
                    Ok(snapshot) => {
                        self.snapshot = snapshot;
                        self.error.clear();
                    }
                    Err(err) => {
                        self.error = err;
                    }
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
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
                requester_id: String::new(),
                security_level: 0,
                result_preview: String::new(),
            }
        );
    }

    /// T-890-06: requester/nível/result previa atravessam o mapeamento,
    /// com resultado truncado para exibição.
    #[test]
    fn tool_row_maps_governance_and_truncates_result() {
        let long = "x".repeat(200);
        let call = ToolCallRequest {
            call_id: String::from("c-2"),
            tool_name: String::from("filesystem.read_file"),
            requester_id: String::from("agent-1"),
            security_level: 2,
            result_json: format!("{{\"conteudo\":\"{long}\"}}"),
            status: 3,
            ..ToolCallRequest::default()
        };

        let row = tool_row(&call);
        assert_eq!(row.requester_id, "agent-1");
        assert_eq!(row.security_level, 2);
        assert!(row.result_preview.ends_with('…'));
        assert!(row.result_preview.chars().count() <= 81);
    }

    #[test]
    fn discovery_row_maps_contract_fields() {
        let event = DiscoveryEvent {
            event_type: String::from("novo"),
            topic_name: String::from("Tasks"),
            remote_entity: String::from("guid-1"),
            ..DiscoveryEvent::default()
        };

        assert_eq!(
            discovery_row(&event),
            DiscoveryRow {
                event_type: String::from("novo"),
                topic_name: String::from("Tasks"),
                remote_entity: String::from("guid-1"),
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

    // T-890: a linha de nó Studio carrega identidade + modo de acesso.
    #[test]
    fn studio_node_row_maps_contract_fields() {
        let presence = StudioNodePresence {
            node_id: String::from("lab-1:4317"),
            url: String::from("http://127.0.0.1:4317"),
            token_required: true,
            ..StudioNodePresence::default()
        };

        assert_eq!(
            studio_node_row(&presence),
            StudioNodeRow {
                node_id: String::from("lab-1:4317"),
                url: String::from("http://127.0.0.1:4317"),
                token_required: true,
            }
        );
    }
}
