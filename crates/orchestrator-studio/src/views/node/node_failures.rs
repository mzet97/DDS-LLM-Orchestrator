//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_failures(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _node_url: &mut String,
    _node_token: &mut String,
    _discovery: &DiscoveryState,
) {
    let status = state.status().to_owned();
    let connected = state.node().is_some();
    let busy = state.busy();
    let (live_label, live_color) = live_state(&status, connected, busy);
    // ── Falhas & diagnóstico de transporte RPC (mockup, textos verdadeiros) ──
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Tratamento de Falhas & Diagnóstico de Transporte RPC")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("ESTADO ATUAL: {live_label}"))
                        .monospace()
                        .small()
                        .strong()
                        .color(live_color),
                );
                ui.label(
                    egui::RichText::new("DIAG_POLICY_V1")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        ui.add_space(theme::SPACE_SM);
        ui.columns(2, |diag| {
            diag[0].vertical(|ui| {
                let active = live_label == "HTTP 401";
                ui.horizontal(|ui| {
                    kit::badge(ui, "HTTP 401", theme::ERROR);
                    ui.label(
                        egui::RichText::new("Unauthorized / Token Inválido")
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    if active {
                        ui.label(
                            egui::RichText::new("● AGORA")
                                .monospace()
                                .small()
                                .strong()
                                .color(theme::AUTH),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "O nó respondeu 401: Bearer ausente ou inválido. O nó está VIVO — \
                         falha de credencial, nunca de rede.",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "Ação do Sistema: insira o token no campo acima e re-conecte; o token \
                         vive só na memória da sessão.",
                    )
                    .small()
                    .color(theme::OUTLINE),
                );
            });
            diag[1].vertical(|ui| {
                let active = live_label == "OFFLINE";
                ui.horizontal(|ui| {
                    kit::badge(ui, "ECONNREFUSED", theme::PRIMARY_FIXED_DIM);
                    ui.label(
                        egui::RichText::new("Falha de Socket TCP / Daemon Offline")
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    if active {
                        ui.label(
                            egui::RichText::new("● AGORA")
                                .monospace()
                                .small()
                                .strong()
                                .color(theme::ERROR),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Socket recusado, host inalcançável ou timeout de 3 s. O daemon pode \
                         estar parado no alvo.",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "Ação do Sistema: confira o studio-node no alvo; a GUI preserva o \
                         último estado válido e tenta de novo na auto-carga.",
                    )
                    .small()
                    .color(theme::OUTLINE),
                );
            });
        });
    });
}
