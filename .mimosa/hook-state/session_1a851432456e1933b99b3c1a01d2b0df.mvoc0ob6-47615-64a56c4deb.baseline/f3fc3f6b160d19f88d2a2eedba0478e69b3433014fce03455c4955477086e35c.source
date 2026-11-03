//! Plano pretendido × efetivo contra stub HTTP real em porta efêmera.

fn stub() -> axum::Router {
    axum::Router::new().route(
        "/services",
        axum::routing::get(|| async {
            axum::Json(serde_json::json!([
                {"service": "a", "wanted": true, "active": false},
                {"service": "b", "wanted": null, "active": true},
            ]))
        }),
    )
}

async fn live_base_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    tokio::spawn(async move {
        axum::serve(listener, stub())
            .await
            .expect("stub de teste serve");
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn parses_wanted_vs_active() {
    let url = live_base_url().await;

    let rows =
        tokio::task::spawn_blocking(move || orchestrator_studio::services::list_services(&url))
            .await
            .expect("sem panic")
            .expect("stub responde");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].service, "a");
    assert_eq!(rows[0].wanted, Some(true));
    assert!(!rows[0].active);
    assert_eq!(rows[1].wanted, None);
    assert!(rows[1].active);
}

#[tokio::test]
async fn actuate_posts_idempotent_operation() {
    use orchestrator_studio::services::actuate;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_route = seen.clone();
    let app = axum::Router::new().route(
        "/services/s/start",
        axum::routing::post(
            |axum::Json(body): axum::Json<serde_json::Value>| async move {
                seen_route
                    .lock()
                    .expect("acessivel")
                    .push(body["operation_id"].as_str().unwrap_or("").to_string());
                axum::Json(serde_json::json!({
                    "service": "s", "wanted": true, "active": true, "acted": true,
                }))
            },
        ),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("stub serve");
    });
    let url = format!("http://{addr}");

    let out = tokio::task::spawn_blocking(move || actuate(&url, "s", true, "op-x"))
        .await
        .expect("sem panic")
        .expect("stub responde");

    assert_eq!(out.service, "s");
    assert!(out.wanted && out.active && out.acted);
    assert_eq!(seen.lock().expect("acessivel").as_slice(), ["op-x"]);
}

#[tokio::test]
async fn unreachable_node_becomes_typed_error() {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let port = probe.local_addr().expect("porta legivel").port();
    drop(probe);

    let err = tokio::task::spawn_blocking(move || {
        orchestrator_studio::services::list_services(&format!("http://127.0.0.1:{port}"))
    })
    .await
    .expect("sem panic")
    .expect_err("porta fechada deve falhar");

    assert!(matches!(
        err,
        orchestrator_studio::services::ServicesError::Unreachable { .. }
    ));
}
