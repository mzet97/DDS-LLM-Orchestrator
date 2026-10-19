//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_header(
    ui: &mut egui::Ui,
    infer: &mut InferenceState,
    discovery: &DiscoveryState,
) {
    let http_live = infer.last_verify_ms.is_some() && !infer.models.is_empty();
    // ── Cabeçalho (mockup; shell global novo já vem do main) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("3.3 Inferência & Chat de Engenharia")
                        .size(22.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                kit::badge(ui, "DDS_TOPIC: Studio.ServerStatus", theme::OUTLINE);
            });
            ui.label(
                egui::RichText::new(
                    "Dois planos: presença DDS (ServerStatus) × chat HTTP (endpoint explícito)",
                )
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("REQ/T-820-19: T_MAX 120s")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if discovery.servers.is_empty() {
                            "○ PLANO DDS: NENHUM SERVIDOR VISTO"
                        } else {
                            "● PLANO DDS: PARTICIPANTE VISTO"
                        })
                        .monospace()
                        .small()
                        .strong()
                        .color(if discovery.servers.is_empty() {
                            theme::STALE
                        } else {
                            theme::PRIMARY_FIXED_DIM
                        }),
                    );
                    ui.label(
                        egui::RichText::new(if http_live {
                            "● PLANO HTTP: ROTAS V1 ATIVAS"
                        } else {
                            "○ PLANO HTTP: ROTAS V1 NÃO VERIFICADAS"
                        })
                        .monospace()
                        .small()
                        .strong()
                        .color(if http_live {
                            theme::OK
                        } else {
                            theme::WARN
                        }),
                    );
                });
            });
        });
    });
    ui.add_space(theme::SPACE_SM);
}
