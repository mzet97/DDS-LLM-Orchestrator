//! Catálogo compartilhado contra a autoridade REAL do nó em porta efêmera.
//!
//! Sem mocks nem stubs: o cliente do Studio fala com o router de verdade do
//! `studio-node` (HTTP de loopback), provando o contrato ponta a ponta.

use orchestrator_studio::catalog_remote::{
    delete, events_since, fetch_snapshot, publish, SharedCatalogError,
};

async fn live_base_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    let state = studio_node::server::NodeState::new(Vec::new());
    tokio::spawn(async move {
        axum::serve(listener, studio_node::server::router(state))
            .await
            .expect("autoridade de teste serve");
    });
    format!("http://{addr}")
}

async fn blocking<F, T>(work: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work).await.expect("sem panic")
}

#[tokio::test]
async fn multi_gui_cycle_through_real_authority() {
    let url = live_base_url().await;

    let rev = blocking({
        let url = url.clone();
        move || publish(&url, "proj-b", None, "v1")
    })
    .await
    .expect("cria");
    assert_eq!(rev, 0);

    let err = blocking({
        let url = url.clone();
        move || publish(&url, "proj-b", Some(7), "v2")
    })
    .await
    .expect_err("base obsoleta deve falhar");
    assert!(
        matches!(err, SharedCatalogError::Conflict { current: Some(0) }),
        "corrente estruturada no fio, recebido: {err:?}"
    );

    let snapshot = blocking({
        let url = url.clone();
        move || fetch_snapshot(&url)
    })
    .await
    .expect("snapshot");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].revision.0, 0);

    blocking({
        let url = url.clone();
        move || delete(&url, "proj-b", 0)
    })
    .await
    .expect("exclui");

    let err = blocking({
        let url = url.clone();
        move || publish(&url, "proj-b", None, "v3")
    })
    .await
    .expect_err("tombstone deve falhar");
    assert!(matches!(err, SharedCatalogError::Tombstoned));

    let events = blocking({
        let url = url.clone();
        move || events_since(&url, 0)
    })
    .await
    .expect("eventos");
    assert_eq!(events.len(), 2);
}
