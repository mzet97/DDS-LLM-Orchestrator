//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn show_band(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _services: &mut ServicesPanel,
    _agents: &mut AgentsState,
    _models: &ModelsState,
    _shared: &mut SharedCatalog,
    _proof: &str,
    discovery: &DiscoveryState,
) {
    let target = discovery.selected_url();
    // ── Faixa de alvo ativo (design: pill fina de uma linha) ──
    let now = now_unix_ns();
    egui::Frame::NONE
        .fill(theme::tint(theme::PRIMARY_CONTAINER, 8))
        .stroke(egui::Stroke::new(
            1.0,
            theme::tint(theme::PRIMARY_CONTAINER, 45),
        ))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .small()
                        .color(theme::PRIMARY_CONTAINER),
                );
                match (&target, state.node()) {
                    (Some(url), Some(node)) => {
                        ui.label(
                            egui::RichText::new(format!("ALVO ATIVO: {url}"))
                                .monospace()
                                .size(11.0)
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "● Conectado ao studio-node v{}.{} · {} ops registradas",
                                node.version.major,
                                node.version.minor,
                                node.operations.len()
                            ))
                            .monospace()
                            .size(11.0)
                            .color(theme::OK),
                        );
                        let ttl = discovery
                            .nodes
                            .iter()
                            .find(|n| n.url == *url)
                            .map(|n| 10u64.saturating_sub(n.age_secs(now)));
                        ui.label(
                            egui::RichText::new(format!(
                                "QoS Lease: 10.0s [TTL: {}]",
                                ttl.map(|t| format!("{t}s"))
                                    .unwrap_or_else(|| String::from("—"))
                            ))
                            .monospace()
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new("Auto-Carga Contínua (5s)")
                                .monospace()
                                .size(11.0)
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new(format!(
                                "○ ALVO ATIVO: — nenhum alvo — · descoberta contínua \
                                 no domínio {}",
                                discovery.domain
                            ))
                            .monospace()
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                }
            });
        });
    ui.add_space(theme::SPACE_SM);

    // ── Hero de descoberta (design: título + Ativo + pill QoS) ──
    let mut heartbeat_ns: Vec<u64> = discovery
        .nodes
        .iter()
        .map(|n| n.last_seen_unix_ns)
        .chain(discovery.agents.iter().map(|a| a.last_update_ns))
        .collect();
    heartbeat_ns.sort_unstable();
    let freshest_age = heartbeat_ns
        .last()
        .map(|newest| now.saturating_sub(*newest) / 1_000_000_000);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("●")
                .size(16.0)
                .color(theme::PRIMARY_CONTAINER),
        );
        ui.label(
            egui::RichText::new(format!(
                "Domínio DDS {} · Descoberta em Tempo Real",
                discovery.domain
            ))
            .size(19.0)
            .strong()
            .color(theme::ON_SURFACE),
        );
        if let Some(age) = freshest_age {
            kit::badge(ui, &format!("● Ativo há {age}s"), theme::OK);
        } else {
            kit::badge(ui, "○ Sem dados", theme::STALE);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            kit::badge(ui, "QoS RELIABLE + TRANSIENT_LOCAL", theme::OUTLINE);
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── 3 cards de descoberta (design: barramento + registry + endpoint) ──
    let node_lines: Vec<(String, String)> = discovery
        .nodes
        .iter()
        .take(3)
        .map(|n| {
            (
                n.node_id.chars().take(21).collect(),
                n.probe
                    .as_ref()
                    .map(|p| p.detail.chars().take(18).collect())
                    .unwrap_or_else(|| String::from("sondando…")),
            )
        })
        .collect();
    let agent_lines: Vec<(String, String)> = discovery
        .agents
        .iter()
        .take(3)
        .map(|a| {
            (
                a.agent_id.chars().take(18).collect(),
                format!("slots {}/{}", a.slots_busy, a.slots_total),
            )
        })
        .collect();
    let server_lines: Vec<(String, String)> = discovery
        .servers
        .first()
        .map(|s| {
            vec![
                (
                    s.server_id.chars().take(18).collect(),
                    if s.ready {
                        String::from("pronto")
                    } else {
                        String::from("não pronto")
                    },
                ),
                (
                    s.model_loaded.chars().take(18).collect(),
                    format!(
                        "slots {}/{}",
                        s.slots_processing,
                        s.slots_idle + s.slots_processing
                    ),
                ),
                (String::from("Publica"), String::from("ServerStatus")),
            ]
        })
        .unwrap_or_else(|| vec![(String::from("—"), String::from("sem presença"))]);
    ui.columns(3, |cols| {
        hero_card(
            &mut cols[0],
            &HeroCard {
                kicker: String::from("TOPOLOGIA DE BARRAMENTO"),
                count: discovery.nodes.len().to_string(),
                name: String::from("Nós Studio"),
                lines: if node_lines.is_empty() {
                    vec![(String::from("—"), String::from("sem nós"))]
                } else {
                    node_lines
                },
            },
        );
        hero_card(
            &mut cols[1],
            &HeroCard {
                kicker: String::from("AGENTREGISTRY"),
                count: discovery.agents.len().to_string(),
                name: String::from("Agentes IA"),
                lines: if agent_lines.is_empty() {
                    vec![(String::from("—"), String::from("sem agentes"))]
                } else {
                    agent_lines
                },
            },
        );
        hero_card(
            &mut cols[2],
            &HeroCard {
                kicker: String::from("SERVERSTATUS ENDPOINT"),
                count: discovery.servers.len().to_string(),
                name: String::from("Servidor Inferência"),
                lines: server_lines,
            },
        );
    });
    ui.add_space(theme::SPACE_MD);
}
