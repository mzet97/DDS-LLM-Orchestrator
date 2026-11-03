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

/// Espera o worker do catálogo ficar ocioso (`busy == false`) dirigindo
/// frames com pausa real. Contagem fixa de frames (`run_steps(N)`) avança
/// mais rápido que o worker HTTP sob carga — e rótulos como "0 itens" já
/// existem antes da operação terminar, então o sinal correto é o `busy`.
fn wait_idle(harness: &mut egui_kittest::Harness<'_, &mut SharedCatalog>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    // Um frame primeiro: o clique só vira `busy` quando a view o processa.
    harness.run_steps(1);
    loop {
        if !harness.state().busy {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "worker nao ficou ocioso"
        );
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn wait_label_contains(harness: &mut egui_kittest::Harness<'_, &mut SharedCatalog>, text: &str) {
    wait_idle(harness);
    harness.get_by_label_contains(text);
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
    wait_label_contains(&mut harness, "2 itens");

    // A remove m:1; B acompanha e converge incrementalmente.
    panel_a.form_id = String::from("m:1");
    panel_a.form_base = String::from("0");
    panel_a.delete_form();
    drain(&mut panel_a).await;
    harness.get_by_label("Acompanhar eventos").click_accesskit();
    wait_label_contains(&mut harness, "TOMBSTONE");
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
    // Snapshot vazio + follow sem eventos: o follow rodou (encadeou).
    wait_label_contains(&mut harness, "sem eventos novos (cursor 0)");
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
