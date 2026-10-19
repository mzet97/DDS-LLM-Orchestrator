//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) const LEASE_SECS: f32 = 10.0;

/// Resumo de uma linha para a tabela de operações do nó.
pub(super) fn op_summary(op: &AdminOp) -> String {
    match op {
        AdminOp::Bootstrap { node_name } => format!("bootstrap {node_name}"),
        AdminOp::SetService { service, running } => {
            format!("serviço {service} {}", if *running { "on" } else { "off" })
        }
    }
}

/// Tipo da operação (coluna da tabela).
pub(super) fn op_kind(op: &AdminOp) -> &'static str {
    match op {
        AdminOp::Bootstrap { .. } => "NODE_BOOTSTRAP",
        AdminOp::SetService { .. } => "SERVICE_ACTION",
    }
}

/// Alvo/contexto da operação (coluna da tabela) — dado real do `AdminOp`.
pub(super) fn op_target(op: &AdminOp) -> String {
    match op {
        AdminOp::Bootstrap { node_name } => node_name.clone(),
        AdminOp::SetService { service, .. } => format!("unit: {service}"),
    }
}

/// Moldura dos cards da 3.2 (mesmo idioma da 3.10).
pub(super) fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

/// Cabeçalho de card: título à esquerda + etiqueta à direita.
pub(super) fn card_title(ui: &mut egui::Ui, title: &str, tag: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tag)
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
}

/// Estado vivo da conexão (linha de status + card de falhas).
pub(super) fn live_state(
    status: &str,
    connected: bool,
    busy: bool,
) -> (&'static str, egui::Color32) {
    if connected {
        ("CONECTADO", theme::OK)
    } else if busy {
        ("SONDANDO", theme::WARN)
    } else if status.contains("negado") {
        ("HTTP 401", theme::AUTH)
    } else if status.contains("bloqueada") {
        ("INCOMPATÍVEL", theme::ERROR)
    } else if status != "sem leitura do nó" {
        ("OFFLINE", theme::ERROR)
    } else {
        ("SEM LEITURA", theme::STALE)
    }
}
