//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_discovery_row(
    ui: &mut egui::Ui,
    machines: &mut MachinesState,
    selected: &mut Option<usize>,
    index: usize,
    node: &DiscoveredNode,
    now_ns: u64,
) {
    let alive = node.is_alive(now_ns);
    let age = node.age_secs(now_ns);
    let is_selected = *selected == Some(index);
    let (state_color, state_dot) = match (&node.probe, alive) {
        (Some(probe), _) => (dot_color(probe.state), "●"),
        (None, true) => (theme::WARN, "○"),
        (None, false) => (theme::STALE, "○"),
    };

    // Coluna 1: estado + node id (+ ALVO PRIMÁRIO / ATIVO).
    ui.horizontal(|ui| {
        if is_selected {
            // Filete ciano da linha selecionada (painter — sem glifo ▌).
            let (edge, _) = ui.allocate_exact_size(egui::vec2(3.0, 36.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(edge, 1.0, theme::PRIMARY_FIXED_DIM);
        }
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(state_dot).size(14.0).color(state_color));
                ui.label(
                    egui::RichText::new(truncate(&node.node_id, 22))
                        .size(13.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
            });
            ui.horizontal(|ui| {
                if is_selected {
                    kit::badge(ui, "ALVO PRIMÁRIO", theme::PRIMARY_FIXED_DIM);
                    ui.label(
                        egui::RichText::new(if alive { "● ATIVO" } else { "○ INATIVO" })
                            .monospace()
                            .small()
                            .color(if alive {
                                theme::PRIMARY_FIXED_DIM
                            } else {
                                theme::STALE
                            }),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("○ DESCOBERTO")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                }
            });
        });
    });

    // Coluna 2: URL (sem GUID — o tópico não a expõe; nada inventado).
    ui.vertical(|ui| {
        ui.hyperlink_to(
            egui::RichText::new(&node.url).monospace().small(),
            &node.url,
        );
    });

    // Coluna 3: probe (protocolo ou motivo honesto).
    match &node.probe {
        Some(probe) if probe.state == ProbeState::Online => {
            ui.label(
                egui::RichText::new(format!("✓ {} (OK)", probe.detail))
                    .monospace()
                    .small()
                    .strong()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
        }
        Some(probe) if probe.state == ProbeState::AuthPending => {
            ui.vertical(|ui| {
                let curto = auth_curto(&probe.detail);
                kit::badge(ui, &format!("● HTTP 401 · {curto}"), theme::AUTH);
                ui.label(
                    egui::RichText::new("Canal RPC aberto · Falha de autorização")
                        .monospace()
                        .small()
                        .color(theme::AUTH),
                );
            });
        }
        Some(probe) => {
            ui.vertical(|ui| {
                let refused = probe.detail.to_lowercase().contains("refus");
                match (refused, port_of(&node.url)) {
                    (true, Some(port)) => {
                        kit::badge(
                            ui,
                            &format!("● Offline (porta {port} recusada)"),
                            theme::ERROR,
                        );
                    }
                    _ => {
                        kit::badge(ui, "● Offline", theme::ERROR);
                    }
                }
                ui.label(
                    egui::RichText::new(truncate(&probe.detail, 48))
                        .monospace()
                        .small()
                        .color(theme::ERROR),
                );
            });
        }
        None => {
            ui.label(egui::RichText::new("sondando…").monospace().small().weak());
        }
    }

    // Coluna 4: autenticação (401 é CREDENCIAL, não rede).
    let has_token = machines.token_for(&node.url).is_some();
    match &node.probe {
        Some(probe) if probe.state == ProbeState::Online => {
            ui.vertical(|ui| {
                if node.token_required {
                    if has_token {
                        ui.label(
                            egui::RichText::new("● Bearer configurado")
                                .monospace()
                                .small()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                        ui.label(egui::RichText::new("(em memória volátil)").small().weak());
                    } else {
                        ui.label(
                            egui::RichText::new("○ sem token em memória")
                                .monospace()
                                .small()
                                .color(theme::WARN),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("○ canal aberto")
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.label(egui::RichText::new("nó sem auth").small().weak());
                }
            });
        }
        Some(probe) if probe.state == ProbeState::AuthPending => {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(if has_token {
                        "× Token recusado"
                    } else {
                        "× Não informado"
                    })
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
                );
                ui.label(egui::RichText::new("Requer Bearer Token").small().weak());
            });
        }
        Some(_) => {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("—")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new("Sem conexão de controle")
                        .small()
                        .weak(),
                );
            });
        }
        None => {
            ui.label(
                egui::RichText::new("—")
                    .monospace()
                    .small()
                    .color(theme::STALE),
            );
        }
    }

    // Coluna 5: heartbeat + ação.
    ui.vertical(|ui| {
        let (hb, hb_color) = heartbeat_label(age);
        ui.label(
            egui::RichText::new(format!("{:.1}s atrás", age_frac(node, now_ns)))
                .monospace()
                .small()
                .color(theme::ON_SURFACE),
        );
        ui.label(
            egui::RichText::new(format!("[{hb}]"))
                .monospace()
                .small()
                .color(hb_color),
        );
        ui.add_space(theme::SPACE_XS);
        if is_selected {
            ui.add_enabled(
                false,
                egui::Button::new(egui::RichText::new("● Alvo Atual").small()),
            );
        } else if matches!(
            &node.probe,
            Some(p) if p.state == ProbeState::AuthPending
        ) {
            let token_btn = kit::primary_button(ui, "Definir Alvo & Inserir Token");
            if token_btn.clicked() {
                machines.modal_url = Some(node.url.clone());
                machines.modal_token.clear();
            }
        } else if matches!(&node.probe, Some(p) if p.state == ProbeState::Online) {
            if ui
                .button(egui::RichText::new("Definir Alvo").small())
                .clicked()
            {
                *selected = Some(index);
                crate::studio_log::info(format!("máquinas: alvo comutado para {}", node.url));
            }
        } else if ui
            .button(egui::RichText::new("↻ Re-testar Conexão").small())
            .clicked()
        {
            machines.probe_url(&node.url);
        }
    });
}
