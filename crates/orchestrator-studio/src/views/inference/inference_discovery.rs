//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_discovery(
    ui: &mut egui::Ui,
    _infer: &mut InferenceState,
    discovery: &DiscoveryState,
) {
    // ── Descoberta de inferência no domínio (mockup) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(
                "● DESCOBERTA DE INFERÊNCIA NO DOMÍNIO (SERVERSTATUS DDS VS. HTTP GATEWAY)",
            )
            .monospace()
            .size(11.5)
            .strong()
            .color(theme::PRIMARY_FIXED_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new("QoS: BestEffort · Volatile · KeepLast(1) (heartbeat)")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if discovery.servers.is_empty() {
            kit::empty_state(
                ui,
                &format!(
                    "Nenhum servidor anunciando ServerStatus no domínio {} — o \
                     llama-server precisa rodar com o DDS ligado para aparecer aqui.",
                    discovery.domain
                ),
            );
        } else {
            kit::table("infer_servers_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "SERVIDOR DESCOBERTO",
                        "MODELO CARREGADO NA VRAM",
                        "KV SLOTS DISPONÍVEIS",
                        "STATUS NO BARRAMENTO DDS",
                    ],
                );
                for server in &discovery.servers {
                    kit::mono_cell(ui, &server.server_id);
                    ui.label(
                        egui::RichText::new(if server.model_loaded.is_empty() {
                            "(não anunciado)"
                        } else {
                            server.model_loaded.as_str()
                        })
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                    let total = server.slots_idle + server.slots_processing;
                    let full = total > 0 && server.slots_idle == 0;
                    ui.label(
                        egui::RichText::new(format!(
                            "{}/{} ATIVOS ({} LIVRES){}",
                            server.slots_processing,
                            total,
                            server.slots_idle,
                            if full {
                                " · LOTADO"
                            } else if server.slots_processing > 0 {
                                " (fila local ativa)"
                            } else {
                                ""
                            }
                        ))
                        .monospace()
                        .small()
                        .color(if full {
                            theme::ERROR
                        } else {
                            theme::ON_SURFACE_VARIANT
                        }),
                    );
                    ui.label(
                        egui::RichText::new(if server.ready {
                            "● Pronto (lease 10s do tópico)"
                        } else {
                            "○ Não pronto"
                        })
                        .monospace()
                        .small()
                        .strong()
                        .color(if server.ready {
                            theme::PRIMARY_FIXED_DIM
                        } else {
                            theme::WARN
                        }),
                    );
                    ui.end_row();
                }
            });
        }
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "Postura de acoplamento §3.3: presença no barramento DDS confirmada \
                 via tópico Studio.ServerStatus (heartbeats BestEffort), mas o \
                 endpoint HTTP exige conexão explícita configurada abaixo. O \
                 contrato ServerStatus não divulga intencionalmente porta/URL \
                 HTTP (isolamento de superfície de ataque): aponte o pipeline \
                 /v1/chat/completions para o endereço RPC do servidor.",
            )
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_LG);
}
