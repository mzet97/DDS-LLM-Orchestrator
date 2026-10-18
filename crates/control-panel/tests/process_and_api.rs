//! Paridade com `test_process_manager.py` + E2E HTTP do supervisor.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use control_panel::catalog::ServiceDefinition;
use control_panel::config::PanelConfig;
use control_panel::process::ManagedProcess;
use control_panel::server::{router, AppState};
use control_panel::supervisor::Supervisor;
use tower::ServiceExt;

fn dummy_definition(log_dir_name: &str) -> (ServiceDefinition, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(log_dir_name);
    std::fs::create_dir_all(&dir).expect("tmpdir");
    (
        ServiceDefinition {
            name: "dummy".into(),
            display_name: "Dummy".into(),
            description: "processo de teste".into(),
            category: "core".into(),
            command: vec![
                "sh".into(),
                "-c".into(),
                "echo control-panel-ok; sleep 30".into(),
            ],
            cwd: dir.clone(),
            env: BTreeMap::new(),
            auto_restart: false,
            depends_on: vec![],
            tags: vec![],
            overridable_args: BTreeMap::new(),
        },
        dir,
    )
}

#[test]
fn managed_process_start_stop() {
    let (def, dir) = dummy_definition("cp-proc-a");
    let mut proc = ManagedProcess::new(
        def.name.clone(),
        def.command.clone(),
        def.cwd.clone(),
        BTreeMap::new(),
        &dir,
    );
    proc.start().expect("start");
    assert!(proc.is_running());
    proc.stop();
    assert!(!proc.is_running());
}

#[test]
fn managed_process_restart_and_log_capture() {
    let (def, dir) = dummy_definition("cp-proc-b");
    let mut proc = ManagedProcess::new(
        def.name.clone(),
        def.command.clone(),
        def.cwd.clone(),
        BTreeMap::new(),
        &dir,
    );
    proc.start().expect("start");
    proc.restart().expect("restart");
    assert!(proc.is_running());
    std::thread::sleep(std::time::Duration::from_millis(300));
    let lines = proc.tail(10);
    assert!(
        lines.iter().any(|l| l.contains("control-panel-ok")),
        "log: {lines:?}"
    );
    proc.stop();
}

#[test]
fn managed_process_double_start_raises_and_stop_idempotent() {
    let (def, dir) = dummy_definition("cp-proc-c");
    let mut proc = ManagedProcess::new(
        def.name.clone(),
        def.command.clone(),
        def.cwd.clone(),
        BTreeMap::new(),
        &dir,
    );
    proc.start().expect("start");
    assert!(proc.start().is_err());
    proc.stop();
    proc.stop();
    assert!(!proc.is_running());
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn api_e2e_start_status_stop() {
    let mut cfg = PanelConfig::from_env();
    cfg.log_dir = std::env::temp_dir().join("cp-api-e2e");
    std::fs::create_dir_all(&cfg.log_dir).expect("logdir");
    let supervisor = Arc::new(Supervisor::new(cfg));
    // Registra um serviço dummy no supervisor pela API de catálogo customizada:
    // usa o start direto do supervisor com definição de teste.
    let (def, _dir) = dummy_definition("cp-api-e2e");
    supervisor
        .start_service(&def, &BTreeMap::new())
        .expect("start dummy");

    let app = router(Arc::new(AppState::new(supervisor.clone())));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("status");
    assert_eq!(response.status(), StatusCode::OK);
    let statuses = body_json(response).await;
    assert!(statuses
        .as_array()
        .expect("array")
        .iter()
        .any(|s| s["name"] == "dummy" && s["running"] == true));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/services/dummy/stop")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("stop");
    assert_eq!(response.status(), StatusCode::OK);
    let stopped = body_json(response).await;
    assert_eq!(stopped["running"], false);
}

#[tokio::test]
async fn ui_root_serves_frontend_html() {
    let cfg = PanelConfig::from_env();
    let supervisor = Arc::new(Supervisor::new(cfg));
    let app = router(Arc::new(AppState::new(supervisor)));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("ui");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let html = String::from_utf8_lossy(&bytes);
    assert!(html.contains("Painel de Controle"));
    assert!(html.contains("href=\"/services\""));
    assert!(html.contains("/api/dashboard"));
}

#[tokio::test]
async fn all_six_pages_exist() {
    let cfg = PanelConfig::from_env();
    let supervisor = Arc::new(Supervisor::new(cfg));
    let app = router(Arc::new(AppState::new(supervisor)));
    for (uri, marker) in [
        ("/", "Presets rápidos"),
        ("/services", "Exportar .log"),
        ("/tests", "Testes disponíveis"),
        ("/chat", "api/chat/send"),
        ("/vms", "Status dos hosts"),
        ("/settings", "Configurações do painel"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("page");
        assert_eq!(response.status(), StatusCode::OK, "pagina {uri}");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let html = String::from_utf8_lossy(&bytes);
        assert!(html.contains(marker), "pagina {uri} sem marcador {marker}");
    }
}

#[tokio::test]
async fn settings_roundtrip_persists_allowed_keys_only() {
    let mut cfg = PanelConfig::from_env();
    cfg.user_config_file =
        std::env::temp_dir().join(format!("cp-settings-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&cfg.user_config_file);
    let supervisor = Arc::new(Supervisor::new(cfg));
    let app = router(Arc::new(AppState::new(supervisor)));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"theme":"light","refresh_interval_ms":9000,"segredo":"nao-salvar"}"#,
                ))
                .expect("req"),
        )
        .await
        .expect("post settings");
    assert_eq!(response.status(), StatusCode::OK);
    let saved = body_json(response).await;
    assert_eq!(
        saved["saved"],
        serde_json::json!(["refresh_interval_ms", "theme"])
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get settings");
    let view = body_json(response).await;
    assert_eq!(view["user_config"]["theme"], "light");
    assert!(view["user_config"].get("segredo").is_none());
}

#[tokio::test]
async fn api_chat_without_dds_feature_streams_error_event() {
    let cfg = PanelConfig::from_env();
    let supervisor = Arc::new(Supervisor::new(cfg));
    let app = router(Arc::new(AppState::new(supervisor)));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat/send")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"prompt":"ola painel"}"#))
                .expect("req"),
        )
        .await
        .expect("chat");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("[DONE]"), "sse: {text}");
    assert!(text.contains("error"), "sse: {text}");
}

#[tokio::test]
async fn api_has_no_auth_anywhere() {
    let cfg = PanelConfig::from_env();
    let supervisor = Arc::new(Supervisor::new(cfg));
    let app = router(Arc::new(AppState::new(supervisor)));
    // Sem autenticação (paridade com o original): dados respondem sem header.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("status sem auth");
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/services/inexistente/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("not found");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
