//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn show_sources(
    ui: &mut egui::Ui,
    state: &mut AppState,
    services: &mut ServicesPanel,
    agents: &mut AgentsState,
    models: &ModelsState,
    shared: &mut SharedCatalog,
    proof: &str,
    discovery: &DiscoveryState,
) {
    // ── Fontes de dados & telemetria (design: título + badge + dots) ──
    let target = discovery.selected_url();
    let hashed = models
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    let (catalog_items, catalog_cursor, catalog_loaded) = match &shared.snapshot {
        Some(snapshot) => (snapshot.items.len(), snapshot.cursor.0, true),
        None => (0, 0, false),
    };
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
        catalog_items,
        catalog_cursor,
        catalog_loaded,
        catalog_error: &shared.notice,
    });
    let ok_count = tiles
        .iter()
        .filter(|tile| !tile.stale && tile.health == TileHealth::Ok)
        .count();
    let warn_count = tiles
        .iter()
        .filter(|tile| !tile.stale && tile.health == TileHealth::Warn)
        .count();
    let stale_count = tiles.len() - ok_count - warn_count;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Fontes de Dados do Sistema & Telemetria")
                .size(15.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        kit::badge(
            ui,
            &format!("{} Sensores Ativos", ok_count + warn_count),
            theme::OUTLINE,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{stale_count} Stale"))
                    .size(11.5)
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("●").size(11.5).color(theme::STALE));
            ui.label(
                egui::RichText::new(format!("{warn_count} Warn"))
                    .size(11.5)
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("●").size(11.5).color(theme::WARN));
            ui.label(
                egui::RichText::new(format!("{ok_count} Ok"))
                    .size(11.5)
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("●").size(11.5).color(theme::OK));
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── 6 cartões ricos (design: título + status, linhas, rodapé) ──
    let dots: Vec<egui::Color32> = tiles
        .iter()
        .map(|tile| health_dot(tile.health, tile.stale))
        .collect();
    let cards = [
        node_card(state, discovery, dots[0]),
        services_card(services, dots[1]),
        agents_card(agents, discovery, dots[2]),
        models_card(models, dots[3]),
        inference_card(discovery, proof, dots[4]),
        catalog_card(shared, &tiles[5].summary, dots[5]),
    ];
    ui.columns(3, |cols| {
        tele_card(&mut cols[0], &cards[0]);
        tele_card(&mut cols[1], &cards[1]);
        tele_card(&mut cols[2], &cards[2]);
    });
    ui.add_space(theme::SPACE_SM);
    ui.columns(3, |cols| {
        tele_card(&mut cols[0], &cards[3]);
        tele_card(&mut cols[1], &cards[4]);
        tele_card(&mut cols[2], &cards[5]);
    });
    ui.add_space(theme::SPACE_LG);
}
