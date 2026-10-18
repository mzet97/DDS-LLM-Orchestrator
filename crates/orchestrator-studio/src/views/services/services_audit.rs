//! Parte de `views::services` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_audit(
    ui: &mut egui::Ui,
    panel: &mut ServicesPanel,
    _guard: &mut ProtectedGuard,
    _discovery: &DiscoveryState,
) {
    // ── Painel de auditoria: atuações + GETs manuais, com relógio ──
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Painel de Auditoria & Log de Operações Systemd").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clear = ui.add_enabled(
                !panel.audit.is_empty(),
                egui::Button::new("Limpar Visualização"),
            );
            if clear.clicked() {
                panel.audit.clear();
            }
            ui.label(
                egui::RichText::new("TRANSPORTE: POST /services/<unidade>/<start|stop>")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    if panel.audit.is_empty() {
        kit::empty_state(ui, "nenhuma atuação registrada nesta sessão.");
    } else {
        egui::Frame::NONE
            .fill(theme::SURFACE_LOW)
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                // Mais recente primeiro (terminal).
                for entry in panel.audit.iter().rev().take(10) {
                    ui.label(
                        egui::RichText::new(format!(
                            "[{}] {} → {}",
                            kit::clock_ms(entry.ts_ms),
                            entry.action,
                            entry.outcome
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                }
            });
    }
}
