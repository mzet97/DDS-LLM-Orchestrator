//! 3.12 Workflow: render dos estados (vazio / concluído / falha) e guarda
//! do botão Executar — via `observe` (sem worker DDS; o corredor real é
//! coberto pelos testes de `workflow.rs` + captura viva).

#![cfg(feature = "dds")]

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::views;
use orchestrator_studio::workflow::{StageOut, WorkflowEvent, WorkflowState};

fn completed_state() -> WorkflowState {
    let mut state = WorkflowState::new();
    state.config.entry = String::from("analisar o log de falha");
    state.busy = true;
    state.observe(WorkflowEvent::Stage(StageOut {
        stage: String::from("A"),
        task_id: String::from("a1b2c3d4-0000-4000-8000-aaaaaaaaaaaa"),
        latency_ms: 142,
        preview: String::from("Identificado erro ECONNREFUSED na porta 4317"),
    }));
    state.observe(WorkflowEvent::Stage(StageOut {
        stage: String::from("B"),
        task_id: String::from("b2c3d4e5-0000-4000-8000-bbbbbbbbbbbb"),
        latency_ms: 1840,
        preview: String::from("Verificar servico systemd no host protegido"),
    }));
    state.observe(WorkflowEvent::Stage(StageOut {
        stage: String::from("C"),
        task_id: String::from("c3d4e5f6-0000-4000-8000-cccccccccccc"),
        latency_ms: 98,
        preview: String::from("Payload formatado com sucesso"),
    }));
    state.observe(WorkflowEvent::Done {
        total_ms: 2080,
        error: None,
    });
    state
}

/// Vazio honesto: faixa, form, cards aguardando, 3 estágios pendentes.
#[test]
fn workflow_empty_renders_form_and_pending() {
    let mut state = WorkflowState::new();
    state.config.entry = String::from("analisar o log de falha");
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, state| views::workflow::show(ui, state),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2600.0));
    harness.run_steps(5);

    harness.get_by_label_contains("EXECUÇÃO DISTRIBUÍDA: §3.12");
    harness.get_by_label_contains("RELIABLE_TRANSIENT_LOCAL");
    harness.get_by_label_contains("Executar Workflow DDS");
    harness.get_by_label_contains("PAYLOAD DE ENTRADA DO DISPARO");
    harness.get_by_label_contains("Aguardando");
    harness.get_by_label("TaskOutput");
    harness.get_by_label_contains("Tasks → TaskOutput · 0/3");
    harness.get_by_label_contains("0/3 entregues");
    harness.get_by_label("A Triagem & Fatos");
    harness.get_by_label("B Síntese & Resolução LLM");
    harness.get_by_label("C Validação & Formatação");
    harness.get_by_label_contains("nenhuma exceção na última execução");
}

/// Concluído: métricas com milhar, cadeia 3/3, auditoria com domínio.
#[test]
fn workflow_completed_renders_chain_and_audit() {
    let mut state = completed_state();
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, state| views::workflow::show(ui, state),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2600.0));
    harness.run_steps(5);

    harness.get_by_label_contains("Concluído");
    harness.get_by_label_contains("Soma de Estágios A (142) + B (1,840) + C (98)");
    harness.get_by_label_contains("3/3 entregues");
    harness.get_by_label_contains("3 / 3 DELIVERED");
    harness.get_by_label_contains("AUDITORIA DE FRAMES DDS NO DOMÍNIO 170");
    harness.get_by_label_contains("total: 2,080 ms (3 estágios encadeados)");
    // Task 8 chars aparece em 2 nós (card + auditoria — mesma fonte).
    let shown = harness.get_all_by_label("a1b2c3d4").count();
    assert_eq!(shown, 2, "task A no card e na auditoria");
}

/// Falha no B: pipeline Falhou, erro real no banner, A segue OK.
#[test]
fn workflow_failure_marks_stage_and_shows_error() {
    let mut state = WorkflowState::new();
    state.config.entry = String::from("analisar o log de falha");
    state.busy = true;
    state.observe(WorkflowEvent::Stage(StageOut {
        stage: String::from("A"),
        task_id: String::from("a1b2c3d4-0000-4000-8000-aaaaaaaaaaaa"),
        latency_ms: 142,
        preview: String::from("Identificado erro ECONNREFUSED"),
    }));
    state.observe(WorkflowEvent::Done {
        total_ms: 15_142,
        error: Some(String::from(
            "estágio B: timeout após 15000 ms (task b2c3d4e5-0000-4000-8000-bbbbbbbbbbbb)",
        )),
    });
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, state| views::workflow::show(ui, state),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2600.0));
    harness.run_steps(5);

    harness.get_by_label_contains("Falhou");
    harness.get_by_label_contains("estágio B: timeout após 15000 ms");
    harness.get_by_label_contains("1/3 entregues");
    harness.get_by_label_contains("1 / 3 DELIVERED");
    // Só A concluiu: selos por card são únicos aqui.
    harness.get_by_label("OK 100%");
    harness.get_by_label("DDS PUB: Tasks");
}

/// Entrada vazia: o clique em Executar não despacha (dupla guarda:
//  botão desabilitado + `show` exige entry não vazio).
#[test]
fn workflow_run_requires_entry() {
    let mut state = WorkflowState::new();
    assert!(state.config.entry.is_empty());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, state| views::workflow::show(ui, state),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2600.0));
    harness.run_steps(5);

    harness
        .get_by_label_contains("Executar Workflow DDS")
        .click();
    harness.run_steps(5);
    assert!(!harness.state().busy, "nada despachado sem entrada");
    assert!(harness.state().stages.is_empty());
}
