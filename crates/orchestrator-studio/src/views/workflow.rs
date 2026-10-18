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
    // Faixa de contexto (espelho do PNG; só fatos do contrato: tópicos
    // Tasks→TaskOutput e QoS efetiva do `Tasks` em `dds_dataspace::qos` —
    // Reliable+TransientLocal KL50. FORA: SCHEMA TaskExecutionChain.idl
    // (não existe) e contagem RTPS de participantes (sem fonte na view).
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(
                "EXECUÇÃO DISTRIBUÍDA: §3.12 WORKFLOW PIPELINE A→B→C / TÓPICOS: \
                 Tasks → TaskOutput / QoS: RELIABLE_TRANSIENT_LOCAL · KL50",
            )
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_XS);
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

/// Formulário em linha (espelho do PNG); devolve `true` quando o botão
/// Executar foi clicado. FORA (não existem): NODE REGISTRY, LEASE LIMIT,
/// ID/ENCODING do disparo e `Tasks/TaskTrigger` (o tópico é `Tasks`).
#[cfg(feature = "dds")]
fn form(ui: &mut egui::Ui, state: &mut WorkflowState) -> bool {
    section(ui, "CONFIGURAÇÃO DO PIPELINE DE ORQUESTRAÇÃO DDS");
    let mut run_clicked = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption("DOMÍNIO DDS"));
            ui.add(egui::DragValue::new(&mut state.config.domain).range(0..=u32::MAX));
        });
        ui.add_space(theme::SPACE_LG);
        ui.vertical(|ui| {
            ui.label(caption("MODELO ALVO (INFERÊNCIA)"));
            ui.add(egui::TextEdit::singleline(&mut state.config.model).desired_width(300.0));
        });
        ui.add_space(theme::SPACE_LG);
        ui.vertical(|ui| {
            ui.label(caption("TIMEOUT GLOBAL"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut state.config.timeout_ms)
                        .speed(1000)
                        .range(1..=u64::MAX),
                );
                ui.label(caption("MS"));
            });
        });
        // right_to_left: primeiro adicionado = mais à direita.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(ui_available(state), |ui| {
                let label = if state.busy {
                    "executando…"
                } else {
                    "▶ Executar Workflow DDS"
                };
                run_clicked = kit::primary_button(ui, label).clicked();
            });
            kit::badge(ui, "CHAIN 3-STAGES", theme::PRIMARY_FIXED_DIM);
        });
    });
    ui.add_space(theme::SPACE_XS);
    ui.label(caption(
        "PAYLOAD DE ENTRADA DO DISPARO / PROMPT INICIAL (TÓPICO: Tasks)",
    ));
    let entry_width = ui.available_width();
    ui.add(
        egui::TextEdit::multiline(&mut state.config.entry)
            .desired_rows(2)
            .desired_width(entry_width)
            .hint_text("texto da tarefa (vira ENTRADA: no prompt do estágio A)"),
    );
    run_clicked
}

/// Rótulo de campo do formulário (mono uppercase discreto, espelho do PNG).
#[cfg(feature = "dds")]
fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

#[cfg(feature = "dds")]
mod workflow_audit;
#[cfg(feature = "dds")]
mod workflow_panels;
#[cfg(feature = "dds")]
mod workflow_run;
#[cfg(any(test, feature = "dds"))]
mod workflow_stage;

#[cfg(feature = "dds")]
use workflow_audit::{audit_table, exceptions, section};
#[cfg(feature = "dds")]
use workflow_panels::{metrics, stage_cards};
#[cfg(feature = "dds")]
use workflow_run::run_chain;
#[cfg(all(test, not(feature = "dds")))]
use workflow_stage::fmt_int;
#[cfg(feature = "dds")]
use workflow_stage::{fmt_int, stage_display};

#[cfg(test)]
mod tests;
