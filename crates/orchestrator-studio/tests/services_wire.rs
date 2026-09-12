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
