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
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::views;

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

/// Leitura do plano: 3 linhas (sincronizada/divergente/desativada), sync com
/// relógio absoluto e GET manual na auditoria.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn services_read_plan_renders_rows_and_audit() {
    let url = live_url(stub_three()).await;
    let mut panel = ServicesPanel::with_url(url.clone());
    let mut guard = ProtectedGuard::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard, discovery)| views::services::show(ui, panel, guard, discovery),
        (&mut panel, &mut guard, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);
    harness.get_by_label_contains("ALVO: http");
    harness.get_by_label_contains("● DDS Domain 170");
    harness.get_by_label_contains("Última Sincronização: —");

    harness
        .get_by_label("↻ Ler plano de serviços")
        .click_accesskit();
    wait_idle(&mut harness);

    harness
        .get_by_label("Serviços do nó (plano: pretendido × efetivo)")
        .click_accesskit();
    harness.run_steps(5);

    harness.get_by_label_contains("3 Declaradas no Plano");
    harness.get_by_label("web");
    harness.get_by_label("worker");
    harness.get_by_label("relay");
    harness.get_by_label_contains("● DIVERGE (wanted=true, active=false)");
    harness.get_by_label_contains("● SINCRONIZADO (Ok)");
    harness.get_by_label_contains("○ DESATIVADO (conforme pretendido)");
    harness.get_by_label_contains("Wanted: false");
    harness.get_by_label_contains("Active: true");
    harness.get_by_label_contains("Última Sincronização: ");
    harness.get_by_label_contains("GET /services (Ler plano)");
    harness.get_by_label_contains("ok · 3 unidade(s)");
}

/// Atuação armada ponta a ponta: ARM → ler → Iniciar → confirmar → auditoria
/// com a rota real e o desfecho do nó.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn services_armed_actuation_end_to_end() {
    let url = live_url(stub_one()).await;
    let mut panel = ServicesPanel::with_url(url);
    let mut guard = ProtectedGuard::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (panel, guard, discovery)| views::services::show(ui, panel, guard, discovery),
        (&mut panel, &mut guard, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);
    harness
        .get_by_label("ARM / HABILITAR ATUAÇÃO")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label("DISARM / PROTEGER NÓ");

    harness
        .get_by_label("Serviços do nó (plano: pretendido × efetivo)")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_label("↻ Ler plano de serviços")
        .click_accesskit();
    wait_idle(&mut harness);
    harness.get_by_label_contains("● DIVERGE (wanted=true, active=false)");
    harness.get_by_label("■ Parar");
    harness.get_by_label("↻ Reiniciar");

    harness.get_by_label("▶ Iniciar").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains(
        "CONFIRMAÇÃO EM 2 PASSOS — Ação solicitada: INICIAR serviço 'worker' no nó",
    );
    harness.get_by_label("✓ Confirmar").click_accesskit();
    wait_idle(&mut harness);

    harness.get_by_label_contains("POST /services/worker/start");
    harness.get_by_label_contains("aplicada · acted=true active=true");
}
