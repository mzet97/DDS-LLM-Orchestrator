//! Painel de visão geral: cartões honestos (§9.2).

use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::overview::{summarize, OverviewInput};
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::state::AppState;

pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    services: &ServicesPanel,
    agents: &AgentsState,
    models: &ModelsState,
    proof: &str,
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
