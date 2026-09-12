//! Painel do catálogo local: botão de leitura + grade de itens.

use eframe::egui;
use orchestrator_studio::state::AppState;
use studio_core::catalog::Catalog;

/// Botão Atualizar (catálogo em memória) + grade ou mensagem honesta.
pub fn show(ui: &mut egui::Ui, catalog: &mut Catalog, state: &mut AppState) {
    if ui.button("Atualizar").clicked() {
        let snapshot = catalog.snapshot();
        state.refresh_from(&snapshot);
    }
    if state.rows().is_empty() {
        ui.label("Nenhum item no catálogo. Use \"Conectar ao nó\" para ler a origem viva.");
    } else {
        egui::Grid::new("catalog_grid").show(ui, |ui| {
            ui.label("id");
            ui.label("valor");
            ui.label("revisão");
            ui.end_row();
            for row in state.rows() {
                ui.label(&row.id);
                ui.label(&row.value);
                ui.label(row.revision.to_string());
                ui.end_row();
            }
        });
    }
}
