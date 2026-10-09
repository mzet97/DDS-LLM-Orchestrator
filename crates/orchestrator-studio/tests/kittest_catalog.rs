//! 3.9 Catálogo: leitura, publicação, 409 com ressincronia, tombstone,
//! follow entre dois painéis, busca/filtros, exportação e feed — contra a
//! autoridade REAL do nó (`studio_node::server::router`, sem mocks).

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::catalog_remote::SharedCatalog;
use orchestrator_studio::views;

async fn live_authority() -> String {
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

/// Drena o worker como a view faria (fora do harness, p/ pré-condições).
async fn drain(shared: &mut SharedCatalog) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while shared.busy {
        assert!(
            tokio::time::Instant::now() < deadline,
            "worker não respondeu a tempo"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        shared.poll();
    }
}

/// Ciclo completo: ler → publicar → editar → tombstone, com tabela, cards e
/// feed acompanhando cada passo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_read_publish_edit_delete_flow() {
    let url = live_authority().await;
    let mut shared = SharedCatalog::with_url(&url);
    shared.form_id = String::from("machine:test-1");
    shared.form_value = String::from("{\"a\":1}");
    shared.form_base.clear();

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, shared| views::shared_catalog::show(ui, shared),
        &mut shared,
    );
    harness.set_size(egui::vec2(1400.0, 2400.0));
    harness.run_steps(5);
    harness.get_by_label_contains("PRIMARY_LEADER: 127.0.0.1:");

    harness.get_by_label("Ler snapshot").click_accesskit();
    harness.run_steps(30);
    harness.get_by_label_contains("rev.0");
    harness.get_by_label_contains("0 itens");

    harness.get_by_label("Publicar Registro").click_accesskit();
    harness.run_steps(40);
    harness.get_by_label("rev.0");
    harness.get_by_label_contains("1 itens");
    harness.get_by_label_contains("Todos (1)");
    harness.get_by_label_contains("REV_COMMITTED");
    harness.get_by_label_contains("rev.0 gravada para machine:test-1");

    harness.get_by_label("Editar").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("registro carregado no formulário");
    harness.get_by_label("Tombstone").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("revise e clique Excluir (tombstone)");

    harness
        .get_by_label("Excluir (tombstone)")
        .click_accesskit();
    harness.run_steps(40);
    harness.get_by_label_contains("TOMBSTONE");
    harness.get_by_label_contains("0 itens");
    harness.get_by_label_contains("1 tombstone(s) nesta sessão");
}

/// 409 real: base errada → banner estruturado + escrita bloqueada →
/// ressincronizar libera o formulário.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_409_conflict_banner_and_resync() {
    let url = live_authority().await;
    // Outro escritor cria cfg:x (rev 0) direto no fio.
    let created: u64 = tokio::task::spawn_blocking({
        let url = url.clone();
        move || orchestrator_studio::catalog_remote::publish(&url, "cfg:x", None, "v1")
    })
    .await
    .expect("sem panic")
    .expect("cria");
    assert_eq!(created, 0);

    let mut shared = SharedCatalog::with_url(&url);
    shared.form_id = String::from("cfg:x");
    shared.form_value = String::from("{\"v\":2}");
    shared.form_base = String::from("7");

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, shared| views::shared_catalog::show(ui, shared),
        &mut shared,
    );
    harness.set_size(egui::vec2(1400.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label("Publicar Registro").click_accesskit();
    harness.run_steps(40);
    harness.get_by_label_contains("HTTP 409 CONFLICT: REVISÃO CONFLITANTE");
    harness.get_by_label_contains("Base: rev.7");
    harness.get_by_label_contains("Servidor: rev.0");
    harness.get_by_label_contains("409_OCC-REJECT");
    harness.get_by_label("Publicar Registro (Desabilitado)");
    harness.get_by_label_contains("base rev.7 < vigente rev.0");

    harness
        .get_by_label("Atualizar Snapshot & Mesclar")
        .click_accesskit();
    harness.run_steps(30);
    // Ressincronizou: botão liberado de volta + linha vigente na tabela.
    harness.get_by_label("Publicar Registro");
    harness.get_by_label("rev.0");
}

/// Busca, filtros, exportação com leitura do JSON e limpeza do feed (tela
/// com snapshot pré-carregado, sem rede).
#[test]
fn catalog_search_filters_export_and_clear() {
    use studio_core::catalog::{Cursor, DefinitionId, ItemView, Snapshot};
    use studio_core::revision::Revision;
    let mut shared = SharedCatalog::with_url("http://127.0.0.1:1");
    shared.snapshot = Some(Snapshot {
        items: vec![
            ItemView {
                id: DefinitionId(String::from("machine:a")),
                value: String::from("{\"x\":1}"),
                revision: Revision(0),
            },
            ItemView {
                id: DefinitionId(String::from("machine:b")),
                value: String::from("{\"x\":2}"),
                revision: Revision(2),
            },
            ItemView {
                id: DefinitionId(String::from("cfg:k")),
                value: String::from("v"),
                revision: Revision(1),
            },
        ],
        cursor: Cursor(9),
    });
    shared.feed = vec![orchestrator_studio::catalog_remote::FeedEntry {
        ts_ms: 50_569_102,
        tag: orchestrator_studio::catalog_remote::FeedTag::Committed,
        text: String::from("rev.0 gravada para machine:a"),
    }];

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, shared| views::shared_catalog::show(ui, shared),
        &mut shared,
    );
    harness.set_size(egui::vec2(1400.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label_contains("rev.9");
    harness.get_by_label_contains("Todos (3)");
    harness.get_by_label_contains("machine (2)");
    harness.get_by_label_contains("cfg (1)");
    harness.get_by_label("machine (2)").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label("machine:a");
    harness.get_by_label_contains("[14:02:49.102]");
    harness.get_by_label_contains("BUFFER: 1 EVTS");

    harness.get_by_label("Exportar JSON").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("snapshot exportado:");
    let back: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::temp_dir().join("studio-catalog-snapshot.json"))
            .expect("relatório existe"),
    )
    .expect("relatório é JSON");
    assert_eq!(back["items"].as_array().map(Vec::len), Some(3));

    harness
        .get_by_label("Limpar visualização")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("nenhum evento acompanhado nesta sessão.");
}

/// Follow entre dois painéis: A tombstone, B acompanha e converge sem
/// re-ler o snapshot manualmente.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_follow_converges_remote_delete() {
    let url = live_authority().await;
    // Painel A (fora do harness): publica 2 itens.
    let mut panel_a = SharedCatalog::with_url(&url);
    for id in ["m:1", "m:2"] {
        panel_a.form_id = String::from(id);
        panel_a.form_value = String::from("{}");
        panel_a.form_base.clear();
        panel_a.publish_form();
        drain(&mut panel_a).await;
    }

    // Painel B (no harness): lê os 2.
    let mut shared = SharedCatalog::with_url(&url);
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, shared| views::shared_catalog::show(ui, shared),
        &mut shared,
    );
    harness.set_size(egui::vec2(1400.0, 2400.0));
    harness.run_steps(5);
    harness.get_by_label("Ler snapshot").click_accesskit();
    harness.run_steps(30);
    harness.get_by_label_contains("2 itens");

    // A remove m:1; B acompanha e converge incrementalmente.
    panel_a.form_id = String::from("m:1");
    panel_a.form_base = String::from("0");
    panel_a.delete_form();
    drain(&mut panel_a).await;
    harness.get_by_label("Acompanhar eventos").click_accesskit();
    harness.run_steps(40);
    harness.get_by_label_contains("TOMBSTONE");
    harness.get_by_label_contains("1 exclusão(ões) aplicada(s)");
    harness.get_by_label_contains("1 itens");
    harness.get_by_label_contains("1 tombstone(s) nesta sessão");
}

/// Forçar Sincronização encadeia snapshot + follow (UpToDate no fim).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_force_sync_chains_follow() {
    let url = live_authority().await;
    let mut shared = SharedCatalog::with_url(&url);
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, shared| views::shared_catalog::show(ui, shared),
        &mut shared,
    );
    harness.set_size(egui::vec2(1400.0, 2400.0));
    harness.run_steps(5);

    harness
        .get_by_label("Forçar Sincronização")
        .click_accesskit();
    harness.run_steps(60);
    // Snapshot vazio + follow sem eventos: o follow rodou (encadeou).
    harness.get_by_label_contains("sem eventos novos (cursor 0)");
}

/// Kind ComboBox reescreve o prefixo do id (sem rede; snapshot local). O
/// kind na tabela é chip painter (sem nó) — a opção do popup é única.
#[test]
fn catalog_kind_combo_rewrites_prefix() {
    use studio_core::catalog::{Cursor, DefinitionId, ItemView, Snapshot};
    use studio_core::revision::Revision;
    let mut shared = SharedCatalog::with_url("http://127.0.0.1:1");
    shared.snapshot = Some(Snapshot {
        items: vec![
            ItemView {
                id: DefinitionId(String::from("machine:a")),
                value: String::from("{}"),
                revision: Revision(0),
            },
            ItemView {
                id: DefinitionId(String::from("cfg:k")),
                value: String::from("v"),
                revision: Revision(1),
            },
        ],
        cursor: Cursor(2),
    });
    shared.form_id = String::from("machine:a");
    shared.form_value = String::from("{}");
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, shared| views::shared_catalog::show(ui, shared),
        &mut shared,
    );
    harness.set_size(egui::vec2(1400.0, 2400.0));
    harness.run_steps(5);
    harness.get_by_value("machine").click();
    harness.run_steps(3);
    harness.get_by_label("cfg").click();
    harness.run_steps(3);
    assert_eq!(harness.state().form_id, "cfg:a");
    harness.get_by_value("cfg");
}
