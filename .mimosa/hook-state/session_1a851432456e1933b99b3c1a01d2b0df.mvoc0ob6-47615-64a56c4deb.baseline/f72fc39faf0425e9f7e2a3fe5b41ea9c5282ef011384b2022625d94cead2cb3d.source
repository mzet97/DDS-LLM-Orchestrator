//! Catálogo compartilhado contra a autoridade REAL do nó em porta efêmera.
//!
//! Sem mocks nem stubs: o cliente do Studio fala com o router de verdade do
//! `studio-node` (HTTP de loopback), provando o contrato ponta a ponta —
//! inclusive o acompanhamento de eventos da GUI (T-830-02).

use orchestrator_studio::catalog_remote::{
    delete, events_since, fetch_snapshot, publish, SharedCatalog, SharedCatalogError,
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

/// Drena o worker do painel como a view faria por frame (REQ/T-820-19),
/// com teto de tempo para falhar rápido se o worker travar.
async fn drain(shared: &mut SharedCatalog) {
    assert!(
        shared.busy,
        "operação deve sinalizar trabalho em background"
    );
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while shared.busy {
        assert!(
            tokio::time::Instant::now() < deadline,
            "worker do catálogo não respondeu a tempo"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        shared.poll();
    }
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

/// T-830-02: mutação de um "segundo cliente" converge na GUI só com o
/// acompanhamento de eventos — sem clique manual em "Ler snapshot".
#[tokio::test]
async fn events_following_converges_created_and_deleted_without_manual_refresh() {
    let url = live_base_url().await;
    let mut shared = SharedCatalog::with_url(&url);

    // Leitura inicial estabelece o cursor local (catálogo vazio, cursor 0).
    shared.refresh();
    drain(&mut shared).await;
    let snapshot = shared.snapshot.as_ref().expect("snapshot inicial");
    assert!(snapshot.items.is_empty());
    assert_eq!(shared.cursor, 0);

    // Segundo cliente publica direto na autoridade.
    let rev = blocking({
        let url = url.clone();
        move || publish(&url, "proj-c", None, "v9")
    })
    .await
    .expect("segundo cliente publica");
    assert_eq!(rev, 0);

    shared.follow_events();
    drain(&mut shared).await;
    let snapshot = shared.snapshot.as_ref().expect("snapshot convergido");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].id.0, "proj-c");
    assert_eq!(snapshot.items[0].value, "v9");
    assert_eq!(snapshot.items[0].revision.0, 0);
    assert_eq!(shared.cursor, 1);
    assert!(
        shared.notice.contains("1 novo(s) evento(s)"),
        "contador de eventos no aviso: {}",
        shared.notice
    );

    // Segundo cliente exclui: lote só de `Deleted` é aplicado incremental.
    blocking({
        let url = url.clone();
        move || delete(&url, "proj-c", 0)
    })
    .await
    .expect("segundo cliente exclui");

    shared.follow_events();
    drain(&mut shared).await;
    let snapshot = shared.snapshot.as_ref().expect("snapshot ainda presente");
    assert!(
        snapshot.items.is_empty(),
        "exclusão aplicada sem snapshot: {snapshot:?}"
    );
    assert_eq!(shared.cursor, 2);
    assert!(shared.notice.contains("exclusão(ões) aplicada(s)"));
}

/// T-830-02: 410 `CursorExpired` cai para o snapshot full e converge.
#[tokio::test]
async fn expired_cursor_falls_back_to_full_snapshot() {
    let url = live_base_url().await;
    let mut shared = SharedCatalog::with_url(&url);
    blocking({
        let url = url.clone();
        move || publish(&url, "proj-d", None, "v1")
    })
    .await
    .expect("seed");

    // Cursor impossivelmente à frente: o nó responde 410 (fora da retenção).
    shared.cursor = u64::MAX;
    shared.follow_events();
    drain(&mut shared).await;

    assert!(
        shared.notice.contains("cursor expirado"),
        "{}",
        shared.notice
    );
    let snapshot = shared.snapshot.as_ref().expect("snapshot reaplicado");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].id.0, "proj-d");
    assert_eq!(shared.cursor, 1, "cursor ressincronizado pelo snapshot");
}

/// T-830-02: nó inalcançável mantém o estado vigente; o motivo vira aviso.
#[tokio::test]
async fn unreachable_authority_keeps_stale_state() {
    let url = live_base_url().await;
    let mut shared = SharedCatalog::with_url(&url);
    blocking({
        let url = url.clone();
        move || publish(&url, "proj-e", None, "v1")
    })
    .await
    .expect("seed");
    shared.refresh();
    drain(&mut shared).await;
    assert_eq!(shared.snapshot.as_ref().expect("estado").items.len(), 1);

    let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let port = probe.local_addr().expect("porta legivel").port();
    drop(probe);
    shared.url = format!("http://127.0.0.1:{port}");

    shared.follow_events();
    drain(&mut shared).await;

    assert!(
        shared.notice.contains("falha no catalogo"),
        "texto do erro tipado preservado: {}",
        shared.notice
    );
    // Estado permanece (stale honesto), nunca linhas inventadas.
    let snapshot = shared.snapshot.as_ref().expect("estado preservado");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].id.0, "proj-e");
}
