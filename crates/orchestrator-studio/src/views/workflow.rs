//! Painel "Workflow": formulário, grade de estágios e corrida A→B→C real.
//!
//! Com a feature `dds`, o corredor injetado em [`WorkflowState::start`]
//! replica o `run_seq` do `wf-run` (mesmos prompts congelados e montagem via
//! `client::wf_assembly`), dentro de um runtime tokio de thread (padrão da
//! GUI: a thread de UI nunca bloqueia). Sem `dds`, o painel informa como
//! habilitar.

use crate::workflow::WorkflowState;
#[cfg(feature = "dds")]
use crate::workflow::{StageOut, WorkflowConfig, WorkflowEvent};
use eframe::egui;

/// Ponto de entrada do painel (feature `dds` — corredor DDS real).
#[cfg(feature = "dds")]
pub fn show(ui: &mut egui::Ui, state: &mut WorkflowState) {
    state.poll();
    ui.heading("Workflow A→B→C (wf-run na GUI)");
    ui.label(
        "Executa a cadeia sequencial canônica pelo data space: cada estágio é \
         uma task DDS real reivindicada por agentes vivos no domínio.",
    );
    ui.separator();
    let run_clicked = form(ui, state);
    if run_clicked && !state.busy && !state.config.entry.trim().is_empty() {
        let config = state.config.clone();
        state.start(move |id, tx| run_chain(id, config, tx));
    }
    results(ui, state);
}

/// Sem DDS: orientação (a máquina de estado continua testável).
#[cfg(not(feature = "dds"))]
pub fn show(ui: &mut egui::Ui, state: &mut WorkflowState) {
    ui.heading("Workflow A→B→C (wf-run na GUI)");
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
            "Executar A→B→C"
        };
        run_clicked = ui.button(label).clicked();
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

#[cfg(feature = "dds")]
fn results(ui: &mut egui::Ui, state: &mut WorkflowState) {
    if state.busy {
        ui.label("executando estágios…");
    }
    if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::RED, format!("erro: {error}"));
    }
    if state.stages.is_empty() && !state.busy {
        ui.label("Nenhum estágio ainda. Preencha a entrada e execute.");
        return;
    }
    egui::Grid::new("workflow_stages").show(ui, |ui| {
        ui.strong("estágio");
        ui.strong("task");
        ui.strong("latência ms");
        ui.strong("conteúdo (prévia)");
        ui.end_row();
        for stage in &state.stages {
            ui.label(&stage.stage);
            ui.monospace(stage.task_id.chars().take(8).collect::<String>());
            ui.label(stage.latency_ms.to_string());
            ui.label(&stage.preview);
            ui.end_row();
        }
    });
    if let Some(total_ms) = state.total_ms {
        if state.error.is_none() {
            ui.label(format!("total: {total_ms} ms (3 estágios encadeados)"));
        }
    }
}
