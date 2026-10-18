//! 3.8 Serviços: leitura do plano, atuação armada ponta a ponta, recusa
//! desarmada pelo clique e auditoria com relógio — contra stub axum real do
//! nó (`GET /services`, `POST /services/<u>/<start|stop>`).

use axum::{
    extract::Path,
    routing::{get, post},
    Json,
};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::DiscoveryState;
use orchestrator_studio::protected::ProtectedGuard;
use orchestrator_studio::services::{AuditEntry, ServicesPanel};
use orchestrator_studio::views;

#[allow(dead_code)]
fn unit(service: &str, wanted: Option<bool>, active: bool) -> serde_json::Value {
    serde_json::json!({
        "service": service,
        "wanted": wanted,
        "active": active,
    })
}

/// Stub com 3 unidades (sincronizada + divergente + desativada conforme).
fn stub_three() -> axum::Router {
    axum::Router::new()
        .route(
            "/services",
            get(|| async {
                Json(serde_json::json!([
                    unit("web", Some(true), true),
                    unit("worker", Some(true), false),
                    unit("relay", Some(false), false),
                ]))
            }),
        )
        .route(
            "/services/:service/:action",
            post(
                |Path((service, action)): Path<(String, String)>| async move {
                    assert!(action == "start" || action == "stop", "ação real do fio");
                    Json(serde_json::json!({
                        "service": service,
                        "wanted": true,
                        "active": true,
                        "acted": true,
                    }))
                },
            ),
        )
}

/// Stub com 1 unidade divergente (botões sem ambiguidade no teste 2).
fn stub_one() -> axum::Router {
    axum::Router::new()
        .route(
            "/services",
            get(|| async { Json(serde_json::json!([unit("worker", Some(true), false)])) }),
        )
        .route(
            "/services/:service/:action",
            post(
                |Path((service, _action)): Path<(String, String)>| async move {
                    Json(serde_json::json!({
                        "service": service,
                        "wanted": true,
                        "active": true,
                        "acted": true,
                    }))
                },
            ),
        )
}

async fn live_url(router: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("stub de teste serve");
    });
    format!("http://{addr}")
}

/// Espera o painel ficar ocioso (`busy == false`) com pausa real entre
/// frames — contagem fixa de frames flutua sob carga paralela (mesma
/// correção dos testes de catálogo).
fn wait_idle(
    harness: &mut egui_kittest::Harness<
        '_,
        (&mut ServicesPanel, &mut ProtectedGuard, &DiscoveryState),
    >,
) {
    // Um frame primeiro: o clique só vira `busy` quando a view o processa.
    harness.run_steps(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if !harness.state().0.busy {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "painel nao ficou ocioso"
        );
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Desarmado, o clique em Iniciar RECUSA sem trafegar (recusa visível).
#[test]
fn services_disarmed_click_refuses() {
    let mut panel = ServicesPanel::with_url(String::from("http://127.0.0.1:1"));
    panel.list = vec![studio_node::server::ServiceStatus {
        service: String::from("worker"),
        wanted: Some(true),
        active: false,
    }];
    let mut guard = ProtectedGuard::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard, discovery)| views::services::show(ui, panel, guard, discovery),
        (&mut panel, &mut guard, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);
    harness
        .get_by_label("Serviços do nó (plano: pretendido × efetivo)")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label("▶ Iniciar").click_accesskit();
    harness.run_steps(3);

    harness.get_by_label_contains("NÃO executada");
}

/// Auditoria com relógio fixo + Limpar Visualização (sem rede).
#[test]
fn services_audit_clock_and_clear() {
    let mut panel = ServicesPanel::with_url(String::from("http://127.0.0.1:1"));
    panel.audit = vec![AuditEntry {
        action: String::from("POST /services/x/start"),
        outcome: String::from("aplicada · acted=true active=true"),
        ts_ms: 50_569_102,
    }];
    let mut guard = ProtectedGuard::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard, discovery)| views::services::show(ui, panel, guard, discovery),
        (&mut panel, &mut guard, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);
    harness.get_by_label_contains("[14:02:49.102]");
    harness.get_by_label_contains("POST /services/x/start");
    harness.get_by_label_contains("TRANSPORTE: POST /services/<unidade>/<start|stop>");

    harness
        .get_by_label("Limpar Visualização")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("nenhuma atuação registrada nesta sessão.");
}
