//! Painel de visão geral: cartões honestos em GRID com estado visual.

use crate::agents::AgentsState;
use crate::discovery::DiscoveryState;
use crate::models::ModelsState;
use crate::overview::{summarize, OverviewInput, TileHealth};
use crate::services::ServicesPanel;
use crate::state::AppState;
use crate::theme;
use eframe::egui;

/// Paleta do painel (escura, consistente com o tema do egui dark).
fn health_style(health: TileHealth, stale: bool) -> (egui::Color32, &'static str) {
    if stale || health == TileHealth::Stale {
        return (egui::Color32::from_rgb(90, 98, 110), "◌"); // apagado
    }
    match health {
        TileHealth::Ok => (egui::Color32::from_rgb(48, 209, 88), "●"), // verde
        TileHealth::Warn => (egui::Color32::from_rgb(255, 214, 10), "◐"), // âmbar
        TileHealth::Stale => (egui::Color32::from_rgb(90, 98, 110), "◌"),
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    services: &ServicesPanel,
    agents: &AgentsState,
    models: &ModelsState,
    proof: &str,
    discovery: &DiscoveryState,
) {
    ui.add_space(theme::SPACE_XS);
    ui.heading("Visão geral");
    ui.weak("Fonte de cada cartão indicada; nada é inventado — apagado = ainda não lido.");
    ui.add_space(theme::SPACE_MD);

    let target = discovery.selected_url();

    // ── Banner Alvo Ativo (3.1): URL + estado do nó + lease em contagem ──
    egui::Frame::NONE
        .fill(theme::tint(theme::PRIMARY_CONTAINER, 10))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::PRIMARY_CONTAINER))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Alvo Ativo:").weak());
                ui.monospace(target.clone().unwrap_or_else(|| String::from("—")));
            });
            ui.horizontal(|ui| match state.node() {
                Some(node) => {
                    ui.label(
                        egui::RichText::new(
                            format!(
                                "Conectado ao studio-node v{}.{} · {} operação(ões) registradas",
                                node.version.major,
                                node.version.minor,
                                node.operations.len()
                            )
                            .to_string(),
                        )
                        .color(theme::OK),
                    );
                }
                None => {
                    ui.label(egui::RichText::new("conectando ao studio-node…").weak());
                }
            });
            // QoS lease do AgentRegistry (10 s) com contagem a partir do
            // heartbeat mais recente visto pela descoberta.
            let now = crate::machines::now_unix_ns();
            let freshest = discovery
                .agents
                .iter()
                .map(|a| a.last_update_ns)
                .max()
                .unwrap_or(0);
            let age = now.saturating_sub(freshest) as f64 / 1e9;
            let ttl = (10.0_f64 - age).max(0.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("QoS Lease:").weak());
                ui.label(
                    egui::RichText::new(format!("10.0s [TTL: {ttl:.1}s]"))
                        .monospace()
                        .color(if ttl > 3.0 { theme::OK } else { theme::WARN }),
                );
                ui.weak("· Auto-Carga Contínua (5s)");
            });
        });
    ui.add_space(theme::SPACE_MD);

    // ── Hero (3.1): descoberta em tempo real, 3 números grandes ──
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_XL)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new("DESCOBERTA EM TEMPO REAL")
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
            ui.add_space(theme::SPACE_SM);
            ui.columns(3, |cols| {
                let big = |ui: &mut egui::Ui, n: usize, label: &str| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(n.to_string())
                                .size(28.0)
                                .strong()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                        ui.label(egui::RichText::new(label).small().color(theme::OUTLINE));
                    });
                };
                big(&mut cols[0], discovery.nodes.len(), "NÓS STUDIO");
                big(&mut cols[1], discovery.agents.len(), "AGENTES IA");
                big(&mut cols[2], discovery.servers.len(), "SERVIDOR INFERÊNCIA");
            });
            if let Some(target) = discovery.selected_url() {
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    egui::RichText::new(format!("🛰 alvo: {target}"))
                        .monospace()
                        .small()
                        .color(theme::SECONDARY_FIXED),
                );
            }
        });
    ui.add_space(theme::SPACE_MD);

    let target = discovery.selected_url();
    let hashed = models
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    let tiles = summarize(&OverviewInput {
        node: state.node(),
        node_error: "",
        services: &services.list,
        services_error: &services.error,
        agents: &agents.list,
        agents_error: &agents.error,
        models_total: models.list.len(),
        models_hashed: hashed,
        inference_proof: proof,
        discovery_nodes: discovery.nodes.len(),
        discovery_agents: discovery.agents.len(),
        discovery_servers: discovery.servers.len(),
        discovery_target: target.as_deref(),
    });

    ui.add_space(theme::SPACE_MD);

    // ── Topologia de Barramento (3.1): os 3 pilares com dados REAIS ──
    ui.columns(3, |cols| {
        // Nós Studio
        cols[0].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").color(theme::OK));
                ui.strong(format!("{} Nós Studio", discovery.nodes.len()));
            });
            ui.add_space(theme::SPACE_XS);
            for node in &discovery.nodes {
                ui.monospace(egui::RichText::new(node.url.clone()).small());
            }
        });
        // AgentRegistry
        cols[1].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🤖").color(theme::PRIMARY_FIXED_DIM));
                ui.strong(format!("{} Agentes IA", discovery.agents.len()));
            });
            ui.add_space(theme::SPACE_XS);
            for agent in &discovery.agents {
                ui.label(
                    egui::RichText::new(format!(
                        "{} · slots {}/{}",
                        agent.agent_id, agent.slots_busy, agent.slots_total
                    ))
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
            }
        });
        // ServerStatus
        cols[2].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚡").color(theme::WARN));
                ui.strong(format!("{} Servidor Inferência", discovery.servers.len()));
            });
            ui.add_space(theme::SPACE_XS);
            for server in &discovery.servers {
                ui.label(
                    egui::RichText::new(format!(
                        "{} · pronto={} · slots {}/{}",
                        server.server_id,
                        if server.ready { "sim" } else { "não" },
                        server.slots_processing,
                        server.slots_idle + server.slots_processing
                    ))
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
            }
        });
    });
    ui.add_space(theme::SPACE_MD);

    // ── Despacho Operacional Imediato (3.1) ──
    ui.collapsing("Despacho Operacional Imediato", |ui| {
        ui.weak(
            "Ações rápidas: abram as seções correspondentes (Topologia re-observa              o domínio; Serviços lê o plano; Nó conecta). A Visão geral é agregado              somente leitura.",
        );
    });

    ui.add_space(theme::SPACE_MD);
    // Grade 2 colunas de cartões com borda colorida por estado.
    egui::Grid::new("overview_grid")
        .spacing(egui::vec2(10.0, 10.0))
        .min_col_width(300.0)
        .show(ui, |ui| {
            for (index, tile) in tiles.iter().enumerate() {
                let (color, icon) = health_style(tile.health, tile.stale);
                ui.group(|ui| {
                    // borda do grupo com a cor do estado
                    ui.set_min_size(egui::vec2(300.0, 74.0));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(icon).color(color).size(16.0));
                            ui.strong(&tile.title);
                        });
                        ui.add_space(2.0);
                        ui.label(if tile.stale {
                            egui::RichText::new(&tile.summary).weak()
                        } else {
                            egui::RichText::new(&tile.summary)
                        });
                    });
                    if tile.stale {
                        // filete à esquerda do grupo quando apagado
                        ui.painter().rect_filled(
                            ui.max_rect().with_max_x(ui.max_rect().left() + 3.0),
                            2.0,
                            egui::Color32::from_rgb(70, 76, 86),
                        );
                    } else {
                        ui.painter().rect_filled(
                            ui.max_rect().with_max_x(ui.max_rect().left() + 3.0),
                            2.0,
                            color,
                        );
                    }
                });
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
            // fecha a linha se ímpar
            if tiles.len() % 2 == 1 {
                ui.end_row();
            }
        });

    ui.add_space(theme::SPACE_MD);
    ui.weak(format!(
        "domínio {}: descoberta automática via Studio.NodePresence / AgentRegistry / ServerStatus",
        discovery.domain
    ));
}
