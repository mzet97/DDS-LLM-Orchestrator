//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

use super::machines_discovery::show_discovery_row;

use super::machines_mesh::draw_mini_mesh;

pub(super) fn show_table(
    ui: &mut egui::Ui,
    machines: &mut MachinesState,
    discovery: &mut DiscoveryState,
    domain: u32,
    now_ns: u64,
) {
    // ── Título da seção + legenda (mockup: legenda à direita) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("INSTALAÇÕES DESCOBERTAS NO BARRAMENTO")
                .size(15.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        kit::badge(ui, "RTPS MULTICAST 239.255.0.1", theme::PRIMARY_FIXED_DIM);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (text, color) in [
                ("Timeout / Porta Recusada", theme::ERROR),
                ("Falha Auth (401)", theme::AUTH),
                ("Autenticado", theme::PRIMARY_FIXED_DIM),
            ] {
                ui.label(egui::RichText::new(text).small().color(color));
                ui.label(egui::RichText::new("●").small().color(color));
            }
        });
    });
    ui.add_space(theme::SPACE_XS);

    // ── Tabela canonical das instalações (mockup 3.10) ──
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhuma instalação vista ainda no domínio {domain} (heartbeats a \
                 cada 5 s; nós com canvas DDS aparecem sozinhos aqui)."
            ),
        );
    } else {
        kit::table("discovery_grid").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Estado / Node ID",
                    "Ponto de Extremidade (URL)",
                    "Probe de Protocolo & Versão",
                    "Autenticação (Bearer)",
                    "Heartbeat & Ações",
                ],
            );
            for (index, node) in discovery.nodes.iter().enumerate() {
                show_discovery_row(ui, machines, &mut discovery.selected, index, node, now_ns);
                ui.end_row();
            }
        });
    }
    ui.add_space(theme::SPACE_LG);

    // ── Linha de 3 cards: lease · classificador · mini-topologia ──
    ui.columns(3, |cols| {
        cols[0].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "JANELA DE LEASE (10S)", "Poda após 30s silêncio");
                ui.add_space(theme::SPACE_XS);
                if discovery.nodes.is_empty() {
                    ui.label(
                        egui::RichText::new("○ sem instalações no domínio")
                            .small()
                            .weak(),
                    );
                }
                for node in &discovery.nodes {
                    let remaining = (LEASE_SECS - age_frac(node, now_ns)).max(0.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(truncate(&node.node_id, 24))
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if remaining > 0.0 {
                                ui.label(
                                    egui::RichText::new(format!("{remaining:.1}s restantes"))
                                        .monospace()
                                        .small()
                                        .color(theme::PRIMARY_FIXED_DIM),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("Expirado (em poda)")
                                        .monospace()
                                        .small()
                                        .color(theme::ERROR),
                                );
                            }
                        });
                    });
                    let bar_width = ui.available_width();
                    ui.add(
                        egui::ProgressBar::new(if remaining > 0.0 {
                            (remaining / LEASE_SECS).clamp(0.0, 1.0)
                        } else {
                            0.06
                        })
                        .desired_width(bar_width)
                        .fill(if remaining > 0.0 {
                            theme::PRIMARY_FIXED_DIM
                        } else {
                            theme::ERROR
                        }),
                    );
                    ui.add_space(theme::SPACE_XS);
                }
            });
        });
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "CLASSIFICADOR DE FALHA SEMÂNTICA", "SEC-DIAG");
                ui.add_space(theme::SPACE_XS);
                let mut auth_failures = 0usize;
                let mut offline = 0usize;
                for node in &discovery.nodes {
                    if let Some(probe) = &node.probe {
                        match probe.state {
                            ProbeState::AuthPending => auth_failures += 1,
                            ProbeState::Offline => offline += 1,
                            _ => {}
                        }
                    }
                }
                ui.columns(2, |diag| {
                    diag[0].vertical(|ui| {
                        ui.label(
                            egui::RichText::new("● HTTP 401 (Auth)")
                                .small()
                                .strong()
                                .color(theme::AUTH),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Conectividade TCP OK, mas token ausente ou \
                                 inválido no nó remoto.",
                            )
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                    diag[1].vertical(|ui| {
                        ui.label(
                            egui::RichText::new("● Offline (Rede)")
                                .small()
                                .strong()
                                .color(theme::ERROR),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Socket recusado ou firewall impedindo a porta \
                                 4317.",
                            )
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                });
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new(format!(
                        "agora: {auth_failures} falha(s) de auth · {offline} \
                         offline"
                    ))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "*Nenhum segredo ou token é exposto em texto simples no \
                         barramento multicast DDS.",
                    )
                    .small()
                    .weak(),
                );
            });
        });
        cols[2].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "TOPOLOGIA DO DOMÍNIO", "MESH SPDP/SEDP");
                ui.add_space(theme::SPACE_XS);
                draw_mini_mesh(ui, &discovery.nodes, discovery.selected, now_ns);
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("SPDP Multi: 239.255.0.1:7400")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("DDS DOMAIN {domain}"))
                                .monospace()
                                .small()
                                .strong()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                    });
                });
            });
        });
    });
    ui.add_space(theme::SPACE_LG);
}
