//! Operações e logs (T11): abas por tipo, timeline, log com pausa e
//! "Parar de acompanhar" quando não há cancelamento real.

use eframe::egui;
use orchestrator_studio::operations::{OperationRecord, OperationsPanel};

/// Filtro por tipo + timeline + visualizador de log.
pub fn show(ui: &mut egui::Ui, panel: &mut OperationsPanel) {
    ui.heading("Operações e execuções");
    ui.horizontal(|ui| {
        ui.label("tipo:");
        ui.text_edit_singleline(&mut panel.kind_filter);
        if ui.button("Limpar").clicked() {
            panel.kind_filter.clear();
        }
    });
    if panel.records.is_empty() {
        ui.label("Nenhuma operação acompanhada. Implantação de servidor e tarefa de inferência têm ciclos distintos.");
    }
    for record in panel.visible() {
        ui.group(|ui| {
            ui.monospace(format!(
                "{} · {} · {} · {}",
                record.id,
                record.kind,
                record.machine,
                record.state.label()
            ));
            for event in &record.events {
                ui.label(&event.detail);
            }
            let action = OperationRecord::follow_action(record.state, record.cancel_supported);
            ui.label(format!("Ação disponível: {action}"));
        });
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("log:");
        ui.checkbox(&mut panel.log.follow, "Acompanhar novas linhas");
        ui.label("pesquisa:");
        ui.text_edit_singleline(&mut panel.log.query);
    });
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            for line in panel.log.visible() {
                ui.monospace(line);
            }
        });
}
