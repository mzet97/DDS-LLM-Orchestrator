//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

use super::topology_mesh::{preview96, task_status_color};
use crate::dds_observe::task_status_label;

/// Aba Tarefas DDS (Tasks ao vivo — ciclo, agente, prioridade, retry).
pub(super) fn tasks_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    selected: &mut Option<String>,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.tasks.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma tarefa no tópico Tasks na janela — despache pela tela 3.6 ou \
             via orquestrador/agente; o ciclo PENDING→ASSIGNED→RUNNING→DONE aparece aqui.",
        );
        return;
    }
    let now = now_unix_ns();
    kit::table("topology_tasks_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Task ID",
                "Estado",
                "Agente",
                "Modelo",
                "Prioridade",
                "Retry",
                "Idade",
            ],
        );
        for task in &snapshot.tasks {
            let status = task_status_label(task.status);
            if !filter.is_empty()
                && !matches(&[
                    &task.task_id,
                    status,
                    &task.assigned_agent,
                    &task.model_name,
                ])
            {
                continue;
            }
            let is_selected = selected.as_deref() == Some(task.task_id.as_str());
            let short_id: String = task.task_id.chars().take(8).collect();
            if ui
                .selectable_label(is_selected, egui::RichText::new(&short_id).monospace())
                .clicked()
            {
                *selected = Some(task.task_id.clone());
            }
            ui.label(
                egui::RichText::new(status)
                    .monospace()
                    .color(task_status_color(task.status)),
            );
            ui.label(if task.assigned_agent.is_empty() {
                "—"
            } else {
                &task.assigned_agent
            });
            ui.label(if task.model_name.is_empty() {
                "—"
            } else {
                &task.model_name
            });
            kit::num_cell(ui, &task.priority.to_string());
            kit::num_cell(ui, &task.retry_count.to_string());
            let age = now.saturating_sub(task.created_at_ns) / 1_000_000_000;
            kit::num_cell(ui, &format!("{age} s"));
            ui.end_row();
        }
    });
}

/// Barra de log de tarefas: `TaskOutput` ao vivo (mais recentes primeiro),
/// com prévia de 96 chars; seleção na aba Tarefas filtra este log.
pub(super) fn task_log_bar(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    selected: &mut Option<String>,
) {
    if snapshot.task_outputs.is_empty() {
        return;
    }
    ui.add_space(theme::SPACE_MD);
    kit::section_label(ui, "Log de tarefas · tópico TaskOutput (janela atual)");
    if let Some(task_id) = selected.clone() {
        ui.horizontal(|ui| {
            let short: String = task_id.chars().take(8).collect();
            ui.label(
                egui::RichText::new(format!("FILTRADO: {short}"))
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("limpar filtro").monospace().small(),
                ))
                .clicked()
            {
                *selected = None;
            }
        });
    }
    let rows: Vec<&crate::dds_observe::TaskOutputRow> = snapshot
        .task_outputs
        .iter()
        .filter(|row| selected.as_ref().is_none_or(|id| row.task_id == *id))
        .collect();
    kit::table("topology_tasklog_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Task ID",
                "Seq",
                "Agente",
                "Final",
                "Tokens",
                "Conteúdo (96 chars)",
            ],
        );
        for row in rows.iter().rev().take(12) {
            let short: String = row.task_id.chars().take(8).collect();
            kit::mono_cell(ui, &short);
            kit::num_cell(ui, &row.seq_num.to_string());
            ui.label(&row.agent_id);
            ui.label(if row.is_final { "✓" } else { "…" });
            kit::num_cell(ui, &row.token_count.to_string());
            kit::mono_cell(ui, &preview96(&row.content));
            ui.end_row();
        }
    });
    if rows.len() > 12 {
        ui.label(
            egui::RichText::new(format!(
                "+{} amostra(s) mais antigas na janela",
                rows.len() - 12
            ))
            .small()
            .weak(),
        );
    }
}
