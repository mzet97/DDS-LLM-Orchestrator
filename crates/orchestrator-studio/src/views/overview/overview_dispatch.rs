//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn show_dispatch(
    ui: &mut egui::Ui,
    state: &mut AppState,
    services: &mut ServicesPanel,
    agents: &mut AgentsState,
    models: &ModelsState,
    shared: &mut SharedCatalog,
    proof: &str,
    discovery: &DiscoveryState,
) {
    let target = discovery.selected_url();
    // ── Despacho operacional imediato (design: rótulo + 2 fantasmas + 1 ciano) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Despacho Operacional Imediato:")
                .size(12.5)
                .color(theme::ON_SURFACE_VARIANT),
        );
        let reload = ui.button("Re-carregar alvo (Nó · Serviços · Catálogo)");
        let read_plan = ui.button("Ler plano de serviços");
        let refresh_orch = kit::primary_button(ui, "↑ Atualizar Orquestrador");
        if reload.clicked() || read_plan.clicked() {
            if let Some(target) = discovery.selected_url() {
                let token =
                    crate::discovery::token_for_url(&target, std::env::var("HOME").ok().as_deref());
                if reload.clicked() {
                    state.refresh_from_node_with_token(&target, token.as_deref());
                }
                services.refresh();
                if reload.clicked() {
                    shared.refresh();
                }
            }
            crate::studio_log::info(String::from("visão geral: releitura do alvo solicitada"));
        }
        if refresh_orch.clicked() {
            agents.refresh();
            crate::studio_log::info(String::from("visão geral: orquestrador atualizado"));
        }
    });
    ui.add_space(theme::SPACE_MD);

    let hashed = models
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    let (catalog_items, catalog_cursor, catalog_loaded) = match &shared.snapshot {
        Some(snapshot) => (snapshot.items.len(), snapshot.cursor.0, true),
        None => (0, 0, false),
    };
    let _tiles = summarize(&OverviewInput {
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
}
