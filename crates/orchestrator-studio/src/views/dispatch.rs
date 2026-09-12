//! Painel de despacho: tarefa real via orquestrador.

use eframe::egui;
use orchestrator_studio::workload::DispatchState;

/// URL, modelo, prompt, botão de despacho e desfecho em texto.
pub fn show(ui: &mut egui::Ui, dispatch: &mut DispatchState) {
    ui.collapsing("Despacho (tarefa real via orquestrador)", |ui| {
        ui.horizontal(|ui| {
            ui.label("orquestrador:");
            ui.text_edit_singleline(&mut dispatch.url);
        });
        ui.horizontal(|ui| {
            ui.label("modelo:");
            ui.text_edit_singleline(&mut dispatch.model);
        });
        ui.label("prompt:");
        ui.text_edit_multiline(&mut dispatch.prompt);
        if ui.button("Despachar e aguardar").clicked() {
            dispatch.send();
        }
        if !dispatch.result.is_empty() {
            ui.label(&dispatch.result);
        }
    });
}
