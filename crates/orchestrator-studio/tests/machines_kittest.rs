//! Interação da tabela de máquinas (T03/UI-2.2): clique seleciona
//! pelo apelido (ID estável) e a seleção permanece após reordenação.
//! Headless via egui_kittest; janela real pertence a UI-6.

use egui_kittest::{kittest::Queryable as _, Harness};
use orchestrator_studio::machines::{show_table, MachineLedger};

#[test]
fn table_click_selects_row_by_alias() {
    let mut ledger = MachineLedger::new();
    ledger
        .add("gpu-amd-01", "http://192.168.1.61:4317")
        .expect("adiciona");
    ledger
        .add("agents-01", "http://192.168.1.62:4317")
        .expect("adiciona");
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut (MachineLedger, Option<String>)| {
            show_table(ui, &state.0, &mut |alias| {
                state.1 = Some(String::from(alias))
            });
        },
        (ledger, None),
    );
    harness.run();
    harness.get_by_label("agents-01").click();
    harness.run();
    assert_eq!(harness.state().1.as_deref(), Some("agents-01"));
}

#[test]
fn table_renders_distinct_dimensions() {
    let mut ledger = MachineLedger::new();
    ledger.add("n1", "http://x").expect("adiciona");
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut (MachineLedger, Option<String>)| {
            show_table(ui, &state.0, &mut |alias| {
                state.1 = Some(String::from(alias))
            });
        },
        (ledger, None),
    );
    harness.run();
    harness.get_by_label("administração");
    harness.get_by_label("confiança");
    harness.get_by_label("evidência DDS");
    harness.get_by_label("não verificado");
    harness.get_by_label("não verificada");
    harness.get_by_label("ausente");
}
