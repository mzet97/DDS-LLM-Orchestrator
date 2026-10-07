//! Etapa B (correções 2026-10-06): verificação headless das telas refeitas —
//! kicker/abas da Topologia 3.11, banner de intertravamento da 3.8 e o 6º
//! cartão (Catálogo) da Visão geral 3.1.

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::catalog_remote::SharedCatalog;
#[cfg(feature = "dds")]
use orchestrator_studio::dds_observe::DdsState;
use orchestrator_studio::discovery::DiscoveryState;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::protected::ProtectedGuard;
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::state::AppState;
use orchestrator_studio::views;
use studio_core::catalog::{Cursor, DefinitionId, ItemView};
use studio_core::revision::Revision;

/// Topologia 3.11 refeita: kicker, contador e abas presentes na árvore.
#[cfg(feature = "dds")]
#[test]
fn topology_panel_renders_kicker_counters_and_tabs() {
    let mut dds = DdsState::new();
    dds.domain = 170;
    let mut discovery = DiscoveryState::new_disabled(170);
    discovery.observe(vec![orchestrator_studio::discovery::DiscoveredNode {
        node_id: String::from("192.168.1.61:4317"),
        url: String::from("http://192.168.1.61:4317"),
        token_required: true,
        last_seen_unix_ns: orchestrator_studio::machines::now_unix_ns(),
        probe: None,
    }]);
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, discovery)| views::topology::show(ui, dds, discovery),
        (&mut dds, &mut discovery),
    );
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);
    harness.get_by_label_contains("TOPOLOGIA DE REDE DDS");
    harness.get_by_label_contains("EVENTOS NA JANELA"); // card de métrica
    harness.get_by_label_contains("Agentes DDS"); // aba de coleções
    harness.get_by_label_contains("Instalações"); // aba de coleções
}

/// Serviços 3.8: banner de intertravamento sempre visível; ARM alterna o
/// estado do guard pela UI (o chip do header continua sendo o outro caminho).
#[test]
fn services_interlock_banner_arms_from_screen() {
    let mut panel = ServicesPanel::with_url(String::from("http://127.0.0.1:1"));
    let mut guard = ProtectedGuard::new();
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard)| views::services::show(ui, panel, guard),
        (&mut panel, &mut guard),
    );
    harness.set_size(egui::vec2(1000.0, 700.0));
    harness.run_steps(5);

    // Banner fora do collapsing: intertravamento visível com botão ARM.
    // (os badges de estado são galley pintado — fora da árvore accesskit —
    // então o estado é afirmado pelo guard e pelo rótulo do botão.)
    harness.get_by_label_contains("INTERTRAVAMENTO DE MODO PROTEGIDO");
    harness.get_by_label("ARM / HABILITAR ATUAÇÃO").click();
    harness.run_steps(3);

    let (_panel, guard) = harness.state();
    assert!(guard.armed, "botão ARM da tela arma o guard");
    harness.get_by_label("DISARM / PROTEGER NÓ");
}

/// Visão geral 3.1: o 6º cartão (Catálogo compartilhado) mostra o snapshot
/// real quando a autoridade respondeu.
#[test]
fn overview_sixth_tile_shows_catalog_snapshot() {
    let mut state = AppState::new(); // sem nó conectado — o cartão Nó fica Warn/Stale
    let mut services = ServicesPanel::with_url(String::from("http://127.0.0.1:1"));
    services.list = vec![studio_node::server::ServiceStatus {
        service: String::from("llama-server"),
        wanted: Some(true),
        active: true,
    }];
    let mut agents = AgentsState::new();
    let models = ModelsState::new();
    let mut shared = SharedCatalog::with_url("http://127.0.0.1:1");
    shared.snapshot = Some(studio_core::catalog::Snapshot {
        items: vec![ItemView {
            id: DefinitionId(String::from("machine:alvo-61")),
            value: String::from(r#"{"kind":"machine"}"#),
            revision: Revision(7),
        }],
        cursor: Cursor(9),
    });

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (state, services, agents, models, shared)| {
            views::overview::show(
                ui,
                state,
                services,
                agents,
                models,
                shared,
                "geração OK",
                &discovery_fallback(),
            )
        },
        (&mut state, &mut services, &mut agents, &models, &mut shared),
    );
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);

    harness.get_by_label_contains("Catálogo compartilhado");
    harness.get_by_label_contains("1 registro(s) no snapshot · cursor 9");
}

/// Descoberta desabilitada (sem feature/worker) para a Visão geral headless.
fn discovery_fallback() -> DiscoveryState {
    DiscoveryState::new_disabled(170)
}
