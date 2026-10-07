//! Painel de visão geral (3.1): agregado honesto do sistema em um relance.

use crate::agents::AgentsState;
use crate::catalog_remote::SharedCatalog;
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::{now_unix_ns, ProbeState};
use crate::models::ModelsState;
use crate::overview::{summarize, OverviewInput, TileHealth};
use crate::panel_header::panel_header;
use crate::services::ServicesPanel;
use crate::state::AppState;
use crate::theme;
use eframe::egui;

/// Paleta dos cartões (tokens do design system).
fn health_style(health: TileHealth, stale: bool) -> (egui::Color32, &'static str) {
    if stale || health == TileHealth::Stale {
        return (theme::STALE, "◌");
    }
    match health {
        TileHealth::Ok => (theme::OK, "●"),
        TileHealth::Warn => (theme::WARN, "◐"),
        TileHealth::Stale => (theme::STALE, "◌"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    services: &mut ServicesPanel,
    agents: &mut AgentsState,
    models: &ModelsState,
    shared: &mut SharedCatalog,
    proof: &str,
    discovery: &DiscoveryState,
) {
    panel_header(
        ui,
        &format!("SEC 3.1 · DOMÍNIO DDS {} · VISÃO GERAL", discovery.domain),
        "Visão geral",
        "Agregado somente leitura — fonte de cada cartão indicada; nada é \
         inventado, apagado = ainda não lido.",
    );

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
                        egui::RichText::new(format!(
                            "Conectado ao studio-node v{}.{} · {} operação(ões) registradas",
                            node.version.major,
                            node.version.minor,
                            node.operations.len()
                        ))
                        .color(theme::OK),
                    );
                }
                None => {
                    ui.label(egui::RichText::new("conectando ao studio-node…").weak());
                }
            });
            // QoS lease do AgentRegistry (10 s) com contagem a partir do
            // heartbeat mais recente visto pela descoberta.
            let now = now_unix_ns();
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

    // ── Botões de ação reais (mockup 3.1): disparam os polls existentes ──
    ui.horizontal(|ui| {
        let reload = ui.button("Re-carregar alvo (Nó · Serviços · Catálogo)");
        let read_plan = ui.button("Ler plano de serviços");
        let refresh_orch = ui.button("Atualizar orquestrador (HTTP)");
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
        }
        if refresh_orch.clicked() {
            agents.refresh();
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

    // ── 3 pilares (mockup 3.1 seção 1): nós · agentes · inferência ──
    ui.columns(3, |cols| {
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
    ui.add_space(theme::SPACE_LG);

    // ── Fontes de dados & telemetria: 6 cartões com legenda real ──
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
        ui.strong("Fontes de Dados do Sistema");
        ui.label(
            egui::RichText::new(format!(
                "● {ok_count} Ok · ◐ {warn_count} Warn · ◌ {stale_count} Stale"
            ))
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_SM);

    egui::Grid::new("overview_grid")
        .spacing(egui::vec2(10.0, 10.0))
        .min_col_width(300.0)
        .show(ui, |ui| {
            for (index, tile) in tiles.iter().enumerate() {
                let (color, icon) = health_style(tile.health, tile.stale);
                ui.group(|ui| {
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
                    // Filete à esquerda com a cor do estado.
                    let stripe_color = if tile.stale { theme::STALE } else { color };
                    ui.painter().rect_filled(
                        ui.max_rect().with_max_x(ui.max_rect().left() + 3.0),
                        0.0,
                        stripe_color,
                    );
                });
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
            if tiles.len() % 2 == 1 {
                ui.end_row();
            }
        });

    ui.add_space(theme::SPACE_LG);

    // ── Instalações no domínio (mockup 3.1 seção 3, campos reais) ──
    ui.strong("Instalações no domínio (Studio.NodePresence)");
    ui.add_space(theme::SPACE_SM);
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhuma instalação vista no domínio {} — nós com canvas DDS \
                 publicam presença sozinhos.",
                discovery.domain
            ),
        );
    } else {
        let now = now_unix_ns();
        egui::Grid::new("overview_nodes_grid")
            .striped(true)
            .show(ui, |ui| {
                kit::grid_header(ui, &["Node ID", "URL", "Idade HB", "Estado"]);
                for node in &discovery.nodes {
                    kit::mono_cell(ui, &node.node_id);
                    kit::mono_cell(ui, &node.url);
                    kit::num_cell(ui, &format!("{} s", node.age_secs(now)));
                    let (color, detail) = match &node.probe {
                        Some(probe) => match probe.state {
                            ProbeState::Online => (theme::OK, probe.detail.clone()),
                            ProbeState::AuthPending => (theme::AUTH, probe.detail.clone()),
                            ProbeState::Offline | ProbeState::Unknown => {
                                (theme::ERROR, probe.detail.clone())
                            }
                        },
                        None => (theme::STALE, String::from("sondando…")),
                    };
                    ui.label(
                        egui::RichText::new(format!("● {detail}"))
                            .monospace()
                            .small()
                            .color(color),
                    );
                    ui.end_row();
                }
            });
    }

    ui.add_space(theme::SPACE_MD);
    ui.weak(format!(
        "domínio {}: descoberta automática via Studio.NodePresence / AgentRegistry / ServerStatus",
        discovery.domain
    ));
}
