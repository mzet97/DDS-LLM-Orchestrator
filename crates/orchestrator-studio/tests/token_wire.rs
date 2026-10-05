//! Clientes HTTP do Studio contra o nó REAL protegido por token
//! (REQ/T-840-03a sobre T-840-01): `fetch_node_summary_with_token`,
//! `fetch_node_version_with_token` e o cliente do catálogo com
//! `Authorization: Bearer` — 401 vira erro tipado com texto distinto
//! "token recusado (401)", nunca um "inalcançável" genérico.

use orchestrator_studio::catalog_remote::{
    fetch_snapshot_with_token, publish_with_token, SharedCatalogError,
};
use orchestrator_studio::origin::{
    fetch_node_summary_with_token, fetch_node_version_with_token, OriginError,
};

const TOKEN: &str = "token-de-teste-forte-123";

/// Nó REAL (router do `studio-node`) em loopback com `STUDIO_NODE_TOKEN`
/// configurado — todas as rotas, exceto `GET /version`, exigem Bearer.
async fn protected_node() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    let state = studio_node::server::NodeState::new(Vec::new()).with_token(TOKEN);
    tokio::spawn(async move {
        axum::serve(listener, studio_node::server::router(state))
            .await
            .expect("nó de teste serve");
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn node_summary_without_token_is_rejected_401() {
    let url = protected_node().await;
    // `/version` fica aberto (T-840-01); é `GET /operations` que recusa.
    let err = tokio::task::spawn_blocking({
        let url = url.clone();
        move || fetch_node_summary_with_token(&url, None)
    })
    .await
    .expect("sem panic")
    .expect_err("sem token deve falhar");
    assert!(matches!(err, OriginError::Unauthorized));
    assert_eq!(err.to_string(), "token recusado (401)");
}

#[tokio::test]
async fn node_summary_with_wrong_token_is_rejected_401() {
    let url = protected_node().await;
    let err = tokio::task::spawn_blocking({
        let url = url.clone();
        move || fetch_node_summary_with_token(&url, Some("token-errado"))
    })
    .await
    .expect("sem panic")
    .expect_err("token errado deve falhar");
    assert!(matches!(err, OriginError::Unauthorized));
}

#[tokio::test]
async fn node_summary_with_correct_token_reads_node() {
    let url = protected_node().await;
    let summary = tokio::task::spawn_blocking({
        let url = url.clone();
        move || fetch_node_summary_with_token(&url, Some(TOKEN))
    })
    .await
    .expect("sem panic")
    .expect("token correto deve ler o nó");
    assert_eq!(
        summary.version,
        studio_node::protocol::NODE_PROTOCOL_VERSION
    );
    assert!(summary.operations.is_empty());
}

/// A sonda de Máquinas (T-840-03c) usa `fetch_node_version_with_token`:
/// contra o nó real `/version` está aberto — responde mesmo sem token
/// (a sonda envia o token quando o tem; nó mais estrito recusaria com 401).
#[tokio::test]
async fn version_probe_answers_open_route_even_without_token() {
    let url = protected_node().await;
    let version = tokio::task::spawn_blocking({
        let url = url.clone();
        move || fetch_node_version_with_token(&url, None)
    })
    .await
    .expect("sem panic")
    .expect("versão");
    assert_eq!(version, studio_node::protocol::NODE_PROTOCOL_VERSION);
}

#[tokio::test]
async fn catalog_without_token_is_rejected_and_with_token_works() {
    let url = protected_node().await;
    let err = tokio::task::spawn_blocking({
        let url = url.clone();
        move || publish_with_token(&url, "proj-t", None, "v1", None)
    })
    .await
    .expect("sem panic")
    .expect_err("publicar sem token deve falhar");
    assert!(
        matches!(err, SharedCatalogError::Unauthorized),
        "recebido: {err:?}"
    );
    assert_eq!(err.to_string(), "token recusado (401)");

    // Snapshot sem token também é recusado (todas as rotas do catálogo).
    let err = tokio::task::spawn_blocking({
        let url = url.clone();
        move || fetch_snapshot_with_token(&url, None)
    })
    .await
    .expect("sem panic")
    .expect_err("snapshot sem token deve falhar");
    assert!(matches!(err, SharedCatalogError::Unauthorized));

    // Com o token correto: publica e lê de volta.
    let rev = tokio::task::spawn_blocking({
        let url = url.clone();
        move || publish_with_token(&url, "proj-t", None, "v1", Some(TOKEN))
    })
    .await
    .expect("sem panic")
    .expect("publica com token");
    assert_eq!(rev, 0);
    let snapshot = tokio::task::spawn_blocking({
        let url = url.clone();
        move || fetch_snapshot_with_token(&url, Some(TOKEN))
    })
    .await
    .expect("sem panic")
    .expect("snapshot com token");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].id.0, "proj-t");
    assert_eq!(snapshot.items[0].value, "v1");
}
