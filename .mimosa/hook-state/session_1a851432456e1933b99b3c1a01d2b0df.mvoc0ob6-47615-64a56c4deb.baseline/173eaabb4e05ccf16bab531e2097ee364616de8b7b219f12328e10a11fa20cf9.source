//! 3.13 Ferramentas: vazio, janela populada, filtro por status, inspetor
//! REQUEST×RESPONSE, exportação JSON e troca de janela — com snapshot
//! montado à mão (sem DDS; o `refresh` real é coberto pela captura viva).

#![cfg(feature = "dds")]

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::dds_observe::{DdsState, ToolRow};
use orchestrator_studio::protected::ProtectedGuard;
use orchestrator_studio::views;

fn guard() -> ProtectedGuard {
    ProtectedGuard {
        armed: false,
        pending: None,
        last_refusal: None,
    }
}

/// Construtor posicional das linhas de teste (9 campos do `ToolRow`).
#[allow(clippy::too_many_arguments)]
fn row(
    call_id: &str,
    tool_name: &str,
    status: i32,
    requester_id: &str,
    security_level: i32,
    preview: &str,
    arguments_json: &str,
    result_json: &str,
    duration_ms: u64,
) -> ToolRow {
    ToolRow {
        call_id: String::from(call_id),
        tool_name: String::from(tool_name),
        status,
        requester_id: String::from(requester_id),
        security_level,
        result_preview: String::from(preview),
        arguments_json: String::from(arguments_json),
        result_json: String::from(result_json),
        duration_ms,
    }
}

fn populated() -> DdsState {
    let mut dds = DdsState::new();
    dds.snapshot.tools = vec![
        row(
            "aa-11111111-0001",
            "bash_exec",
            2,
            "agent-qwen-coder",
            2,
            "DENIED (Bloqueado por RBAC)",
            "{\"command\":\"rm -rf /tmp/x\"}",
            "{\"denied\":\"rbac\"}",
            4,
        ),
        row(
            "bb-22222222-0002",
            "read_systemd_status",
            4,
            "agent-mistral",
            0,
            "active (running)",
            "{\"unit\":\"studio-node\"}",
            "{\"active\":\"running\"}",
            840,
        ),
        row(
            "cc-33333333-0003",
            "compile_syntax_check",
            3,
            "agent-qwen-coder",
            1,
            "aguardando saida",
            "{\"file\":\"main.rs\"}",
            "",
            0,
        ),
        row(
            "dd-44444444-0004",
            "git_diff_inspect",
            0,
            "agent-qwen-coder",
            0,
            "na fila",
            "{\"repo\":\"tese\"}",
            "",
            0,
        ),
    ];
    dds
}

/// Janela vazia: orientação honesta + cards zerados + controles.
#[test]
fn tools_empty_shows_guidance_and_zero_cards() {
    let dds = DdsState::new();
    let mut state = (dds, guard());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label_contains("Nenhuma tool call na janela");
    harness.get_by_label("N0 READ_ONLY");
    harness.get_by_label("N1 SANDBOX_EXEC");
    harness.get_by_label("N2 HOST_MUTATION");
    harness.get_by_value("Últimos 5 segundos");
    harness.get_by_label("Observar Domínio");
    harness.get_by_label("Exportar JSON");
}

/// Janela populada: filtros contam, taxa e bloqueios honestos, tabela cheia.
#[test]
fn tools_populated_renders_filters_rate_and_table() {
    let mut state = (populated(), guard());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label("Todos (4)");
    harness.get_by_label("DENIED (1)");
    harness.get_by_label("EXECUTING (1)");
    harness.get_by_label("COMPLETED (1)");
    harness.get_by_label("PENDING (1)");
    harness.get_by_label_contains("TAXA 0.8 REQ/s (janela)");
    harness.get_by_label_contains("BLOQUEIOS (DENIED na janela): 1");
    harness.get_by_label("AUDITORIA DE TOOL CALLS EM TEMPO REAL");
    harness.get_by_label("111-0001");
    harness.get_by_label("bash_exec");
    harness.get_by_label("agent-mistral");
    harness.get_by_label("840ms");
}

/// Filtro DENIED: só a bloqueada permanece (a concluída some de verdade).
#[test]
fn tools_filter_denied_narrows_table() {
    let mut state = (populated(), guard());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label("DENIED (1)").click();
    harness.run_steps(3);
    assert_eq!(harness.state().0.tools_filter, Some(2));
    harness.get_by_label("111-0001");
    let gone = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        harness.get_by_label("222-0002");
    }));
    assert!(gone.is_err(), "concluída some sob filtro DENIED");
}

/// Seleção abre o inspetor com payloads íntegros + COPIAR RAW.
#[test]
fn tools_select_opens_inspector_with_raw_payloads() {
    let mut state = (populated(), guard());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label("111-0001").click();
    harness.run_steps(3);
    harness.get_by_label_contains("INSPETOR DE CHAMADA SELECIONADA: 111-0001 · DENIED");
    harness.get_by_label("PAYLOAD DE ENTRADA (arguments_json)");
    harness.get_by_label("RESULTADO (result_json)");
    // Multiline expõe pai MultilineTextInput + filho TextRun com o mesmo
    // value (adapter egui-accesskit 0.36): 2 nós = payload renderizado.
    assert_eq!(
        harness
            .get_all_by_value("{\"command\":\"rm -rf /tmp/x\"}")
            .count(),
        2
    );
    assert_eq!(harness.get_all_by_value("{\"denied\":\"rbac\"}").count(), 2);
    harness.get_by_label("COPIAR RAW");
}

/// Exportar JSON escreve a janela e o arquivo relê 4 calls.
#[test]
fn tools_export_writes_readable_json() {
    let mut state = (populated(), guard());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_label("Exportar JSON").click();
    harness.run_steps(3);
    let back: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::temp_dir().join("studio-tools.json"))
            .expect("relatório existe"),
    )
    .expect("relatório é JSON");
    assert_eq!(back["tools"].as_array().map(Vec::len), Some(4));
    assert_eq!(back["domain"], serde_json::Value::from(170));
}

/// Janela ComboBox troca 5s → 30s (taxa recalcula).
#[test]
fn tools_window_combo_switches_secs() {
    let mut state = (populated(), guard());
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        &mut state,
    );
    harness.set_size(egui::vec2(1600.0, 2400.0));
    harness.run_steps(5);

    harness.get_by_value("Últimos 5 segundos").click();
    harness.run_steps(3);
    harness.get_by_label("Últimos 30 segundos").click();
    harness.run_steps(3);
    assert_eq!(harness.state().0.window_secs, 30);
    harness.get_by_value("Últimos 30 segundos");
    harness.get_by_label_contains("TAXA 0.1 REQ/s (janela)");
}
