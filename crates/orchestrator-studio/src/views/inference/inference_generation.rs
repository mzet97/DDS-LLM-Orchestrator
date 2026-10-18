//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_generation(
    ui: &mut egui::Ui,
    infer: &mut InferenceState,
    discovery: &DiscoveryState,
) {
    // ── Geração em andamento (mockup; só com `busy` real) ──
    if let Some(started) = infer.pending_since {
        let elapsed = started.elapsed().as_secs_f32();
        card_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new(format!(
                        "Geração em Andamento: {elapsed:.1}s (Timeout máx: 120s · REQ/T-820-19)"
                    ))
                    .small()
                    .strong()
                    .color(theme::ON_SURFACE),
                );
            });
            ui.label(
                egui::RichText::new(match discovery.servers.first() {
                    Some(server) => format!(
                        "Aguardando tokens… · FILA: {} idle · {} processing",
                        server.slots_idle, server.slots_processing
                    ),
                    None => String::from("Aguardando tokens… · FILA: sem ServerStatus no domínio"),
                })
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
        ui.add_space(theme::SPACE_SM);
    }
}
