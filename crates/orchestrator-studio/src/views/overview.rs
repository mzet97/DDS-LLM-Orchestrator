//! Painel de visão geral: cartões honestos (§9.2).

use crate::agents::AgentsState;
use crate::discovery::DiscoveryState;
use crate::models::ModelsState;
use crate::overview::{summarize, OverviewInput};
use crate::services::ServicesPanel;
use crate::state::AppState;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    services: &ServicesPanel,
    agents: &AgentsState,
    models: &ModelsState,
    proof: &str,
    discovery: &DiscoveryState,
) {
    ui.heading("Visão geral (somente leitura)");
    ui.label("Cada cartão mostra a fonte; apagado = ainda não lido, sem dado inventado.");
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
        discovery_target: discovery.selected_url().as_deref(),
    });
    for tile in tiles {
        ui.group(|ui| {
            ui.strong(&tile.title);
            let text = if tile.stale {
                egui::RichText::new(&tile.summary).weak()
            } else {
                egui::RichText::new(&tile.summary)
            };
            ui.label(text);
        });
    }
}
