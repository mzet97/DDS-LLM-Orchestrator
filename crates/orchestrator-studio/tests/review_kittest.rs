//! Interação da revisão (T12/UI-4.3): carregar exemplo rotulado,
//! ver base × local × remota e decidir sem sobrescrita automática.
//! Headless via egui_kittest; janela real pertence a UI-6.

use egui_kittest::{kittest::Queryable as _, Harness};
use orchestrator_studio::review::{ConflictDecision, ReviewState};

#[test]
fn review_demo_loads_labeled_conflict_and_decides() {
    let mut harness = Harness::new_ui_state(
        |ui, review: &mut ReviewState| orchestrator_studio::review::show(ui, review),
        ReviewState::new("", 0),
    );
    harness.run();
    harness
        .get_by_label("Carregar exemplo (demonstração)")
        .click();
    harness.run();
    harness.get_by_label_contains("agente-demo (demonstração)");
    harness.get_by_label("Conflito: remoto andou sob sua edição.");
    harness.get_by_label("Manter rascunho").click();
    harness.run();
    assert_eq!(harness.state().decision, ConflictDecision::KeepDraft);
    assert_eq!(harness.state().conflicts().len(), 1);
}
