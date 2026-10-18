//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_cards(
    ui: &mut egui::Ui,
    state: &mut AppState,
    node_url: &mut String,
    _node_token: &mut String,
    discovery: &DiscoveryState,
) {
    let status = state.status().to_owned();
    let connected = state.node().is_some();
    let _busy = state.busy();
    let (protocol, ops_count) = state
        .node()
        .map(|node| {
            (
                format!("{}.{}", node.version.major, node.version.minor),
                node.operations.len(),
            )
        })
        .unwrap_or_else(|| (String::from("—"), 0));
    let now_ns = now_unix_ns();
    let lease = discovery
        .nodes
        .iter()
        .find(|node| node.url == *node_url)
        .map(|node| {
            let age = now_ns.saturating_sub(node.last_seen_unix_ns) as f32 / 1_000_000_000.0;
            (age, (LEASE_SECS - age).max(0.0))
        });
    // ── Linha de 3 cards: protocolo · operações · concessão/lease ──
    ui.columns(3, |cols| {
        cols[0].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let blocked = status.contains("bloqueada");
                card_title(
                    ui,
                    "VERSÃO DO PROTOCOLO",
                    if connected {
                        "COMPATÍVEL §31"
                    } else if blocked {
                        "BLOQUEADO §31"
                    } else {
                        "SEM LEITURA"
                    },
                );
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new(format!("v{protocol}"))
                        .monospace()
                        .size(20.0)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new("Protocolo RPC do studio-node")
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "Handshake /version: {}",
                        if connected {
                            "OK"
                        } else if blocked {
                            "INCOMPATÍVEL"
                        } else {
                            "—"
                        }
                    ))
                    .monospace()
                    .small()
                    .color(if connected {
                        theme::OK
                    } else if blocked {
                        theme::ERROR
                    } else {
                        theme::OUTLINE
                    }),
                );
            });
        });
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "TOTAL DE OPERAÇÕES", "CACHE LOCAL: OK");
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new(ops_count.to_string())
                        .monospace()
                        .size(20.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                ui.label(
                    egui::RichText::new("registradas no nó")
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new("GET /operations · espelho íntegro")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        cols[2].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "MODO DO NÓ & CONCESSÃO", "ALVO GLOBAL");
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new("● Nó alvo")
                        .size(14.0)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                match lease {
                    Some((age, remaining)) => {
                        ui.label(
                            egui::RichText::new(format!(
                                "Lease DDS 10s · {remaining:.1}s restantes"
                            ))
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new(format!("Último heartbeat: {age:.1}s atrás"))
                                .monospace()
                                .small()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("URL manual (sem lease DDS)")
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new("Último heartbeat: —")
                                .monospace()
                                .small()
                                .weak(),
                        );
                    }
                }
            });
        });
    });
    ui.add_space(theme::SPACE_LG);
}
