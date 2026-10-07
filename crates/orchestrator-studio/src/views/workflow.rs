//! Painel 3.12 Workflow (pipeline A→B→C): formulário, métricas cumulativas,
//! três cards de estágio com conectores (estado por estágio), tabela de
//! auditoria e painel de exceção com o erro REAL do estágio que falhou.
//!
//! Com a feature `dds`, o corredor injetado em [`WorkflowState::start`]
//! replica o `run_seq` do `wf-run` (mesmos prompts congelados e montagem via
//! `client::wf_assembly`), dentro de um runtime tokio de thread (padrão da
//! GUI: a thread de UI nunca bloqueia). Sem `dds`, o painel informa como
//! habilitar.

#[cfg(feature = "dds")]
use crate::kit;
use crate::panel_header::panel_header;
#[cfg(feature = "dds")]
use crate::theme;
use crate::workflow::WorkflowState;
#[cfg(feature = "dds")]
use crate::workflow::{StageOut, WorkflowConfig, WorkflowEvent};
use eframe::egui;

/// Ponto de entrada do painel (feature `dds` — corredor DDS real).
#[cfg(feature = "dds")]
pub fn show(ui: &mut egui::Ui, state: &mut WorkflowState) {
    state.poll();
    panel_header(
        ui,
        "SEC 3.12 · WORKFLOW PIPELINE A→B→C · CADEIA DDS",
        "Workflow (A→B→C)",
        "Cadeia sequencial canônica pelo data space: cada estágio é uma task \
         DDS real reivindicada por agentes vivos · tópico Tasks · saída Tasks/TaskOutput",
    );

    let run_clicked = form(ui, state);
    if run_clicked && !state.busy && !state.config.entry.trim().is_empty() {
        let config = state.config.clone();
        state.start(move |id, tx| run_chain(id, config, tx));
    }
    metrics(ui, state);
    stage_cards(ui, state);
    audit_table(ui, state);
    exceptions(ui, state);
}

/// Sem DDS: orientação (a máquina de estado continua testável).
#[cfg(not(feature = "dds"))]
pub fn show(ui: &mut egui::Ui, state: &mut WorkflowState) {
    panel_header(
        ui,
        "SEC 3.12 · WORKFLOW PIPELINE A→B→C · CADEIA DDS",
        "Workflow (A→B→C)",
        "Cadeia sequencial canônica pelo data space",
    );
    ui.label(
        "Requer a feature `dds` na compilação do Studio (cargo build \
         --features dds) e agentes vivos no domínio informado.",
    );
    let _ = state;
}

#[cfg(feature = "dds")]
fn ui_available(state: &WorkflowState) -> bool {
    !state.busy && !state.config.entry.trim().is_empty()
}

/// Formulário; devolve `true` quando o botão Executar foi clicado.
#[cfg(feature = "dds")]
fn form(ui: &mut egui::Ui, state: &mut WorkflowState) -> bool {
    kit::section_label(ui, "Configuração do Pipeline de Orquestração DDS");
    egui::Grid::new("workflow_form").show(ui, |ui| {
        ui.label("domínio DDS:");
        ui.add(egui::DragValue::new(&mut state.config.domain).range(0..=u32::MAX));
        ui.end_row();
        ui.label("entrada:");
        ui.add(
            egui::TextEdit::singleline(&mut state.config.entry)
                .desired_width(420.0)
                .hint_text("texto da tarefa (vira ENTRADA: no prompt A)"),
        );
        ui.end_row();
        ui.label("modelo:");
        ui.text_edit_singleline(&mut state.config.model);
        ui.end_row();
        ui.label("timeout (ms):");
        ui.add(egui::DragValue::new(&mut state.config.timeout_ms).speed(1000));
        ui.end_row();
    });
    let mut run_clicked = false;
    ui.add_enabled_ui(ui_available(state), |ui| {
        let label = if state.busy {
            "executando…"
        } else {
            "Executar Workflow (A→B→C)"
        };
        run_clicked = ui
            .add(egui::Button::new(
                egui::RichText::new(label)
                    .monospace()
                    .color(theme::ON_PRIMARY),
            ))
            .clicked();
    });
    run_clicked
}

#[cfg(feature = "dds")]
fn run_chain(workflow_id: u64, config: WorkflowConfig, tx: std::sync::mpsc::Sender<WorkflowEvent>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            let _ = tx.send(WorkflowEvent::Done {
                total_ms: 0,
                error: Some(format!("runtime tokio: {err}")),
            });
            return;
        }
    };
    let outcome = rt.block_on(chain(workflow_id, config, &tx));
    let _ = tx.send(outcome);
}

/// Réplica do `run_seq` do `wf-run` com os MESMOS prompts congelados.
#[cfg(feature = "dds")]
async fn chain(
    workflow_id: u64,
    config: WorkflowConfig,
    tx: &std::sync::mpsc::Sender<WorkflowEvent>,
) -> WorkflowEvent {
    use client::dds_impl::DdsClientDds;
    use client::wf_assembly::{self, ANALYSIS, REVIEW};
    use client::{ClientConfig, DdsClient};
    use std::time::Instant;

    let started = Instant::now();
    let config_client = ClientConfig {
        client_id: format!("studio-wf{workflow_id}"),
        dds_domain: config.domain,
        timeout_ms: config.timeout_ms,
    };
    let helper = DdsClient::new(config_client.clone());
    let client = match DdsClientDds::new(config_client) {
        Ok(client) => client,
        Err(err) => {
            return WorkflowEvent::Done {
                total_ms: started.elapsed().as_millis() as u64,
                error: Some(format!("cliente DDS: {err}")),
            };
        }
    };
    let mut prior: Vec<String> = Vec::new();
    let stages = [
        ("A", wf_assembly::SEQ_PROMPTS[0], vec![]),
        ("B", wf_assembly::SEQ_PROMPTS[1], vec![ANALYSIS]),
        ("C", wf_assembly::SEQ_PROMPTS[2], vec![ANALYSIS, REVIEW]),
    ];
    for (name, prompt, labels) in stages {
        let carried: Vec<(&str, String)> = labels
            .iter()
            .zip(prior.iter())
            .map(|(label, content)| (*label, content.clone()))
            .collect();
        let messages = wf_assembly::build_messages(prompt, &config.entry, &carried);
        let task = helper.create_task(&config.model, &messages, 5, false);
        match client.submit(task).await {
            Ok(result) if result.success => {
                let _ = tx.send(WorkflowEvent::Stage(StageOut {
                    stage: String::from(name),
                    task_id: result.task_id.clone(),
                    latency_ms: result.latency_ms,
                    preview: WorkflowState::preview(&result.content, 96),
                }));
                prior.push(result.content);
            }
            Ok(result) => {
                return WorkflowEvent::Done {
                    total_ms: started.elapsed().as_millis() as u64,
                    error: Some(format!(
                        "estágio {name} sem sucesso (task {})",
                        result.task_id
                    )),
                };
            }
            Err(err) => {
                return WorkflowEvent::Done {
                    total_ms: started.elapsed().as_millis() as u64,
                    error: Some(format!("estágio {name}: {err}")),
                };
            }
        }
    }
    WorkflowEvent::Done {
        total_ms: started.elapsed().as_millis() as u64,
        error: None,
    }
}

/// Estado visual de um estágio (derivação honesta do estado real).
#[cfg(feature = "dds")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum StageVisual {
    Pendente,
    Executando,
    Concluido,
    Falhou,
}

#[cfg(feature = "dds")]
fn stage_of<'a>(stages: &'a [StageOut], name: &str) -> Option<&'a StageOut> {
    stages.iter().find(|stage| stage.stage == name)
}

#[cfg(feature = "dds")]
fn stage_visual(state: &WorkflowState, name: &str) -> StageVisual {
    if stage_of(&state.stages, name).is_some() {
        return StageVisual::Concluido;
    }
    if let Some(error) = &state.error {
        if error.contains(&format!("estágio {name}")) {
            return StageVisual::Falhou;
        }
    }
    if state.busy {
        // Primeiro estágio sem card = o que está executando agora.
        let first_missing = ["A", "B", "C"]
            .iter()
            .find(|candidate| stage_of(&state.stages, candidate).is_none());
        if first_missing == Some(&name) {
            return StageVisual::Executando;
        }
    }
    StageVisual::Pendente
}

/// Faixa de 4 métricas do pipeline (mockup 3.12).
#[cfg(feature = "dds")]
fn metrics(ui: &mut egui::Ui, state: &mut WorkflowState) {
    ui.add_space(theme::SPACE_MD);
    let breakdown = ["A", "B", "C"]
        .iter()
        .map(|name| {
            stage_of(&state.stages, name)
                .map(|stage| stage.latency_ms.to_string())
                .unwrap_or_else(|| "—".to_owned())
        })
        .collect::<Vec<_>>()
        .join(" + ");
    let pipeline_state = if state.busy {
        ("Em execução", theme::WARN)
    } else if state.error.is_some() {
        ("Falhou", theme::ERROR)
    } else if state.total_ms.is_some() {
        ("Concluído com sucesso", theme::OK)
    } else {
        ("Aguardando execução", theme::STALE)
    };
    let delivered = state.stages.len();
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Tempo total cumulativo",
            state
                .total_ms
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "—".to_owned()),
            &format!("A {breakdown} ms"),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Estado do pipeline",
            pipeline_state.0.to_owned(),
            if state.busy {
                "estágios em andamento"
            } else {
                "sem retry silencioso"
            },
            pipeline_state.1,
        );
        kit::metric_card(
            &mut cols[2],
            "Saída final",
            format!("{delivered}/3 entregues"),
            "tópico Tasks/TaskOutput",
            if delivered == 3 {
                theme::OK
            } else if delivered > 0 {
                theme::WARN
            } else {
                theme::STALE
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Último erro",
            if state.error.is_some() {
                "sim".to_owned()
            } else {
                "nenhum".to_owned()
            },
            if state.busy { "executando…" } else { "ver exceções abaixo" },
            if state.error.is_some() {
                theme::ERROR
            } else {
                theme::OK
            },
        );
    });
    ui.add_space(theme::SPACE_MD);
}

/// Cadeia distribuída: 3 cards de estágio com conectores (mockup 3.12).
#[cfg(feature = "dds")]
fn stage_cards(ui: &mut egui::Ui, state: &mut WorkflowState) {
    kit::section_label(ui, "Cadeia distribuída em execução (A → B → C)");
    let visuals: [(String, StageVisual); 3] = [
        (String::from("A"), stage_visual(state, "A")),
        (String::from("B"), stage_visual(state, "B")),
        (String::from("C"), stage_visual(state, "C")),
    ];
    // Conectores A→B→C com o estágio de origem concluído destacado.
    let connector = |from: StageVisual| -> (egui::Color32, &'static str) {
        match from {
            StageVisual::Concluido => (theme::OK, "●——▶"),
            StageVisual::Falhou => (theme::ERROR, "●——✖"),
            _ => (theme::STALE, "◌——▶"),
        }
    };
    ui.horizontal(|ui| {
        for (index, (name, visual)) in visuals.iter().enumerate() {
            if index > 0 {
                let (_, previous) = visuals[index - 1].clone();
                let (color, symbol) = connector(previous);
                ui.label(
                    egui::RichText::new(symbol)
                        .monospace()
                        .color(color)
                        .size(16.0),
                );
            }
            let (accent, state_label) = match visual {
                StageVisual::Concluido => (theme::OK, "● CONCLUÍDO"),
                StageVisual::Executando => (theme::WARN, "◐ EXECUTANDO"),
                StageVisual::Falhou => (theme::ERROR, "✖ FALHOU"),
                StageVisual::Pendente => (theme::STALE, "◌ PENDENTE"),
            };
            kit::accent_card(ui, accent, |ui| {
                ui.set_min_width(230.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).strong().size(16.0));
                    kit::badge(ui, state_label, accent);
                });
                match stage_of(&state.stages, name) {
                    Some(stage) => {
                        ui.label(
                            egui::RichText::new(format!(
                                "task {}",
                                stage.task_id.chars().take(8).collect::<String>()
                            ))
                            .monospace()
                            .small(),
                        );
                        ui.label(egui::RichText::new(format!(
                            "latência: {} ms",
                            stage.latency_ms
                        ))
                        .monospace()
                        .small());
                        ui.label(
                            egui::RichText::new(format!(
                                "prévia do buffer (96): {}",
                                if stage.preview.is_empty() {
                                    "—"
                                } else {
                                    &stage.preview
                                }
                            ))
                            .small()
                            .weak(),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("aguardando o estágio anterior…")
                                .small()
                                .weak(),
                        );
                    }
                }
            });
        }
    });
    ui.add_space(theme::SPACE_MD);
}

/// Tabela de auditoria dos frames entregues (mesma fonte dos cards).
#[cfg(feature = "dds")]
fn audit_table(ui: &mut egui::Ui, state: &mut WorkflowState) {
    if state.stages.is_empty() {
        return;
    }
    kit::section_label(ui, &format!(
        "Auditoria de estágios DDS · {}/3 ENTREGUES",
        state.stages.len()
    ));
    egui::Grid::new("workflow_stages")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(
                ui,
                &["Estágio", "Task ID", "Latência", "Buffer (prévia 96 chars)"],
            );
            for stage in &state.stages {
                ui.label(egui::RichText::new(&stage.stage).strong());
                kit::mono_cell(
                    ui,
                    &stage.task_id.chars().take(8).collect::<String>(),
                );
                ui.label(format!("{} ms", stage.latency_ms));
                ui.label(egui::RichText::new(&stage.preview).small().weak());
                ui.end_row();
            }
        });
    if let Some(total_ms) = state.total_ms {
        if state.error.is_none() {
            ui.label(
                egui::RichText::new(format!(
                    "total: {total_ms} ms (3 estágios encadeados)"
                ))
                .monospace()
                .small()
                .color(theme::OK),
            );
        }
    }
    ui.add_space(theme::SPACE_MD);
}

/// Tratamento de exceções: o erro REAL do estágio que falhou.
#[cfg(feature = "dds")]
fn exceptions(ui: &mut egui::Ui, state: &mut WorkflowState) {
    kit::section_label(ui, "Tratamento de exceções & timeout por estágio");
    if let Some(error) = &state.error {
        kit::error_banner(
            ui,
            &format!("{error} — o pipeline para no primeiro estágio que falha (sem retry silencioso)"),
        );
    } else if state.busy {
        ui.label(
            egui::RichText::new("executando — falhas aparecem aqui com o motivo real")
                .small()
                .weak(),
        );
    } else {
        ui.label(
            egui::RichText::new("nenhuma exceção na última execução")
                .small()
                .color(theme::OK),
        );
    }
}
