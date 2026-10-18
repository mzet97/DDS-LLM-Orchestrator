//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_header(
    ui: &mut egui::Ui,
    _state: &mut AppState,
    _node_url: &mut String,
    _node_token: &mut String,
    _discovery: &DiscoveryState,
) {
    // ── Cabeçalho SEC:3.2 (mockup; shell global novo já vem do main) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                kit::badge(ui, "SEC:3.2", theme::PRIMARY_FIXED_DIM);
                ui.label(
                    egui::RichText::new("3.2 Nó studio-node")
                        .size(22.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .size(12.0)
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(
                        "Daemon Local por Máquina (Porta 4317) · Protocolo RPC & Log de Operações",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            kit::badge(ui, "RUST_EGUI_RUNTIME", theme::OUTLINE);
        });
    });
    ui.add_space(theme::SPACE_SM);
}
