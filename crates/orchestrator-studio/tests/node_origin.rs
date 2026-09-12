//! Origem remota contra servidor real em porta efêmera (sem mocks).

use orchestrator_studio::state::AppState;
use studio_node::server::{router, NodeState};

async fn live_base_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    let state = NodeState::new(vec![String::from("dds-agent")]);
    tokio::spawn(async move {
        axum::serve(listener, router(state))
            .await
            .expect("servidor de teste serve");
    });
    format!("http://{addr}")
}

async fn fetch_blocking(
    url: String,
) -> Result<orchestrator_studio::origin::NodeSummary, orchestrator_studio::origin::OriginError> {
    tokio::task::spawn_blocking(move || orchestrator_studio::origin::fetch_node_summary(&url))
        .await
        .expect("tarefa de fetch nao pode sofrer panic")
}

#[tokio::test]
async fn fetch_reads_live_version_and_empty_operations() {
    let url = live_base_url().await;

    let summary = fetch_blocking(url).await.expect("no local deve responder");

    assert_eq!(
        summary.version,
        studio_node::protocol::NODE_PROTOCOL_VERSION
    );
    assert!(summary.operations.is_empty());
}

/// `refresh_from_node` bloqueia (thread de UI do eframe não tem runtime);
/// nos testes, isola em `spawn_blocking` como a produção exige.
async fn refresh_blocking(mut state: AppState, url: String) -> AppState {
    tokio::task::spawn_blocking(move || {
        state.refresh_from_node(&url);
        state
    })
    .await
    .expect("refresh nao pode sofrer panic")
}

#[tokio::test]
async fn app_state_shows_node_summary_and_survives_unreachable() {
    let url = live_base_url().await;

    let state = refresh_blocking(AppState::new(), url).await;

    let node = state.node().expect("conectado deve expor resumo");
    assert!(state.status().contains("protocolo 1.0"));
    assert!(state.status().contains("0 operação(ões)"));
    assert!(node.operations.is_empty());

    let closed = {
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback deve ligar");
        let port = probe.local_addr().expect("porta legivel").port();
        drop(probe);
        format!("http://127.0.0.1:{port}")
    };
    let state = refresh_blocking(state, closed).await;

    assert!(state.node().is_none());
    assert!(state.status().contains("inalcançável"));
}
