//! Painel de despacho: tarefa real via orquestrador.

use crate::panel_header;
use crate::workload::DispatchState;
use eframe::egui;

/// URL, modelo, prompt, botão de despacho e desfecho em texto.
pub fn show(ui: &mut egui::Ui, dispatch: &mut DispatchState) {
    panel_header::panel_header(
        ui,
        "SEC 3.6 · DESPACHO — TASK SÍNCRONA HTTP :8080",
        "Despacho",
        "Task síncrona direto ao orquestrador: resposta com agente executor, latência e motivo de falha tipado",
    );
    // Drena o worker de despacho (thread + mpsc — REQ/T-820-19).
    dispatch.poll();
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
        ui.add_enabled_ui(!dispatch.busy, |ui| {
            if ui.button("Despachar e aguardar").clicked() {
                dispatch.send();
            }
        });
        if !dispatch.result.is_empty() {
            ui.label(&dispatch.result);
        }
    });
}
