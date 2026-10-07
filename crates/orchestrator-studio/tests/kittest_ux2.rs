//! T-890-UX2 (PRD v1.0): verificação headless dos deltas da rodada UX2:
//! aba Tarefas DDS e log de TaskOutput (3.11), inspetor REQUEST×RESPONSE
//! (3.13), colunas de subsistema/slot no buffer (3.14) e decomposição
//! fila×geração (3.6).
//!
//! As telas 3.11/3.13 vivem atrás da feature `dds` (o binário de release
//! a carrega); sem ela este arquivo compila vazio, como `dds_live.rs`.
#![cfg(feature = "dds")]

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::dds_observe::{TaskOutputRow, TaskRow, ToolRow};
use orchestrator_studio::discovery::DiscoveryState;
use orchestrator_studio::protected::ProtectedGuard;
use orchestrator_studio::studio_log::LogsPanel;
use orchestrator_studio::views;
use orchestrator_studio::workload::{DispatchOutcome, DispatchState, LatencyBreakdown};

/// 3.11: a primeira aba é TAREFAS DDS; tarefas injetadas renderizam com ID
/// 8-hex + ciclo, e o log de TaskOutput aparece no rodapé (prévia 96).
#[cfg(feature = "dds")]
#[test]
fn topology_tasks_tab_renders_tasks_and_output_log() {
    let mut dds = orchestrator_studio::dds_observe::DdsState::new();
    dds.tab = 0; // TAREFAS DDS é a aba 0
    dds.snapshot.tasks = vec![TaskRow {
        task_id: String::from("task-8f21a4bc-xyz"),
        client_id: String::from("studio"),
        assigned_agent: String::from("agent-1"),
        model_name: String::from("qwen3.5"),
        status: 2,
        priority: 5,
        retry_count: 0,
        created_at_ns: 1,
        completed_at_ns: 0,
    }];
    dds.snapshot.task_outputs = vec![TaskOutputRow {
        task_id: String::from("task-8f21a4bc-xyz"),
        seq_num: 1,
        agent_id: String::from("agent-1"),
        is_final: false,
        token_count: 12,
        emitted_at_ns: 2,
        content: String::from("parte inicial da resposta do estágio"),
    }];
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, discovery)| views::topology::show(ui, dds, discovery),
        (&mut dds, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);

    harness.get_by_label_contains("Tarefas DDS"); // aba 0
    harness.get_by_label_contains("Server Status"); // aba nova
    harness.get_by_label("RUNNING"); // ciclo canônico da tarefa
    harness.get_by_label_contains("LOG DE TAREFAS"); // rodapé TaskOutput
    harness.get_by_label_contains("parte inicial da resposta"); // prévia
                                                                // Filtro por tópico canônico (chips com contagem real).
    harness.get_by_label_contains("Tasks (1)");
    harness.get_by_label_contains("TaskOutput (1)");
}

/// 3.13: inspetor mostra REQUEST (arguments_json) × RESPONSE (result_json)
/// lado a lado com os payloads íntegros do fio.
#[test]
fn tools_inspector_shows_request_and_response_payloads() {
    let mut dds = orchestrator_studio::dds_observe::DdsState::new();
    dds.snapshot.tools = vec![ToolRow {
        call_id: String::from("call-1"),
        tool_name: String::from("fs.read"),
        status: 4,
        requester_id: String::from("agent-1"),
        security_level: 0,
        result_preview: String::from("{\"ok\":true…"),
        arguments_json: String::from("{\"path\":\"/etc/hostname\"}"),
        result_json: String::from("{\"ok\":true,\"bytes\":13}"),
        duration_ms: 42,
    }];
    dds.tools_selected = Some(String::from("call-1"));
    let mut guard = ProtectedGuard::new();

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dds, guard)| views::tools::show(ui, dds, guard),
        (&mut dds, &mut guard),
    );
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);

    // (section_label uppercaseia; o conteúdo dos TextEdit não é label
    // accesskit — o payload íntegro é coberto pelo teste unitário de ToolRow.
    // "N0 READ_ONLY" aparece no card E na tabela → ambíguo p/ get_by_label;
    // o vocabulário é garantido por security_level_labels_follow_prd.)
    harness.get_by_label_contains("REQUEST (ARGUMENTS_JSON)");
    harness.get_by_label_contains("RESPONSE (RESULT_JSON)");
    harness.get_by_label_contains("COPIAR RAW");
    harness.get_by_label("42 ms"); // duração real (completed−created)
}

/// 3.14: a tabela carrega SUBSISTEMA e SLOT #; entradas marcadas
/// preservam a origem para o inspetor lateral.
#[test]
fn logs_table_carries_subsystem_and_slot() {
    orchestrator_studio::studio_log::info_at(
        "SYSTEMD",
        Some(String::from(r#"{"unit":"llama-server"}"#)),
        "teste ux2 subsistema",
    );
    let mut panel = LogsPanel::new();

    let mut harness =
        egui_kittest::Harness::new_ui_state(|ui, panel| views::logs::show(ui, panel), &mut panel);
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);

    harness.get_by_label_contains("SLOT #"); // header da tabela
    harness.get_by_label_contains("SUBSISTEMA");
    harness.get_by_label("SYSTEMD");
    harness.get_by_label_contains("teste ux2 subsistema");
}

/// 3.6: o card de resultado decompõe a latência nos componentes reais
/// T1–T6 do `/sync` (fila × geração × transporte × serial).
#[test]
fn dispatch_result_card_decomposes_latency() {
    let mut dispatch = DispatchState::new();
    dispatch.last = Some(DispatchOutcome::Completed {
        task_id: String::from("t-1"),
        assigned_agent: Some(String::from("agent-1")),
        latency_ms: 1_500,
        content: Some(String::from("ok")),
        breakdown: Some(LatencyBreakdown {
            queue_ms: 900,
            inference_ms: 400,
            transport_ms: 150,
            serial_ms: 3,
        }),
    });
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dispatch, discovery)| views::dispatch::show(ui, dispatch, discovery),
        (&mut dispatch, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);

    harness.get_by_label_contains(
        "TOTAL 1500 ms = FILA 900 + GERAÇÃO 400 + TRANSPORTE 150 + SERIAL 3",
    );
    harness.get_by_label_contains("TAREFA CONCLUÍDA");
}
