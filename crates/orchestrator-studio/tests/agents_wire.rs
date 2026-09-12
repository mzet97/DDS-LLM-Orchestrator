//! Agentes contra stub HTTP real em porta efêmera.

fn stub() -> axum::Router {
    axum::Router::new().route(
        "/api/v1/agents",
        axum::routing::get(|| async {
            axum::Json(serde_json::json!({"agents": [{
                "agent_id": "agent-teste",
                "model": "qwen3.5-0.8b",
                "specialization": "Text",
                "hostname": "host-teste",
                "health": 2,
                "slots_busy": 1,
                "slots_total": 8,
                "completed_total": 16,
                "failed_total": 1,
                "ema_latency_ms": 1659.75,
            }]}))
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
async fn parses_live_agents_payload() {
    let url = live_base_url().await;

    let agents =
        tokio::task::spawn_blocking(move || orchestrator_studio::agents::list_agents(&url))
            .await
            .expect("sem panic")
            .expect("stub responde");

    assert_eq!(agents.len(), 1);
    let agent = &agents[0];
    assert_eq!(agent.agent_id, "agent-teste");
    assert_eq!(agent.model, "qwen3.5-0.8b");
    assert_eq!(agent.slots_busy, 1);
    assert_eq!(agent.slots_total, 8);
    assert_eq!(agent.completed_total, 16);
    assert_eq!(agent.failed_total, 1);
    assert!(agent.ema_latency_ms > 1000.0);
}

#[tokio::test]
async fn unreachable_orchestrator_becomes_typed_error() {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let port = probe.local_addr().expect("porta legivel").port();
    drop(probe);

    let err = tokio::task::spawn_blocking(move || {
        orchestrator_studio::agents::list_agents(&format!("http://127.0.0.1:{port}"))
    })
    .await
    .expect("sem panic")
    .expect_err("porta fechada deve falhar");

    assert!(matches!(
        err,
        orchestrator_studio::agents::AgentsError::Unreachable { .. }
    ));
}
