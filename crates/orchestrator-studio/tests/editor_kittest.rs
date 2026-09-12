//! Interação do assistente de agente (T05/UI-3.1): digitar nome,
//! avançar com validação e voltar preservando campos — pelos widgets.
//! Headless via egui_kittest; janela real pertence a UI-6.

use egui::accesskit::Role;
use egui_kittest::{
    kittest::{AccessKitNode, Queryable as _},
    Harness,
};
use orchestrator_studio::agent_editor::{AgentEditor, EditorStep};

/// Primeiro campo de texto do passo em ordem de renderização (nome do
/// agente na Identidade).
fn name_field<'tree>(harness: &'tree Harness<'tree, AgentEditor>) -> egui_kittest::Node<'tree> {
    let mut all = harness.get_all_by(|node: &AccessKitNode<'_>| node.role() == Role::TextInput);
    all.next().expect("campo nome existe")
}

#[test]
fn wizard_types_name_advances_and_goes_back() {
    let mut harness = Harness::new_ui_state(
        |ui, editor: &mut AgentEditor| orchestrator_studio::agent_editor::show(ui, editor),
        AgentEditor::new("def-w1"),
    );
    harness.run();
    name_field(&harness).click();
    harness.run();
    name_field(&harness).type_text("revisor");
    harness.run();
    harness.get_by_label("Avançar").click();
    harness.run();
    assert_eq!(harness.state().step, EditorStep::Execution);
    assert_eq!(harness.state().draft.name, "revisor");
    harness.get_by_label("Voltar").click();
    harness.run();
    assert_eq!(harness.state().step, EditorStep::Identity);
    assert_eq!(harness.state().draft.name, "revisor");
}

#[test]
fn wizard_empty_name_blocks_with_local_error() {
    let mut harness = Harness::new_ui_state(
        |ui, editor: &mut AgentEditor| orchestrator_studio::agent_editor::show(ui, editor),
        AgentEditor::new("def-w2"),
    );
    harness.run();
    harness.get_by_label("Avançar").click();
    harness.run();
    assert_eq!(harness.state().step, EditorStep::Identity);
    harness.get_by_label("Corrija: nome do agente é obrigatório");
}
