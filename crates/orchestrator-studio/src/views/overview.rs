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

    ui.add_space(6.0);
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
