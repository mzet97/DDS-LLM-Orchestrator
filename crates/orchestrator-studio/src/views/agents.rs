//! Painel 3.5 Agentes: fiel ao mockup Stitch — duas fontes com semântica
//! distinta (FONTE 1: DDS/AgentRegistry ao vivo × FONTE 2: orquestrador HTTP
//! auxiliar), Seção A com barras de slots/lease, Seção B com taxas reais,
//! agregado do enxame e snapshot JSON.
//!
//! Só dados reais: sem GUID/NIC/MTU/p95/jitter de agente (o fio não tem),
//! sem estratégia de despacho por agente (não anunciada), sem fila global ou
//! policy (inexistentes no contrato). O QoS `TRANSIENT_LOCAL / RELIABLE` e a
//! poda de 30 s vêm do CÓDIGO (`qos.rs`, `discovery.rs`). O `TOPIC_ID` hexa
//! do mockup é floreio inventado — omitido.

use crate::agents::AgentsState;
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::now_unix_ns;
use crate::theme;
use eframe::egui;

/// Taxa de sucesso honesta: concluídas/(concluídas+falhas); `None` sem dados.
fn success_rate(completed: u64, failed: u64) -> Option<f64> {
    let total = completed + failed;
    (total > 0).then(|| completed as f64 / total as f64 * 100.0)
}

/// Instante ms unix → "14:02:11.412" (UTC, conta manual).
fn hhmmss_ms(ts_unix_ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (ts_unix_ms / 3_600_000) % 24,
        (ts_unix_ms / 60_000) % 60,
        (ts_unix_ms / 1_000) % 60,
        ts_unix_ms % 1_000
    )
}

/// Moldura dos cards da 3.5 (mesmo idioma das demais).
fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

pub fn show(ui: &mut egui::Ui, agents: &mut AgentsState, discovery: &DiscoveryState) {
    agents.poll();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));

    show_header(ui, agents, discovery);
    show_registry(ui, agents, discovery);
    show_orchestrator(ui, agents, discovery);
}

mod agents_header;
mod agents_orchestrator;
mod agents_registry;

use agents_header::show_header;
use agents_orchestrator::show_orchestrator;
use agents_registry::show_registry;

#[cfg(test)]
mod tests;
