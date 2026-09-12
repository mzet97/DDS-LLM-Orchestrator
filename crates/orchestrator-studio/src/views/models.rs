//! Painel de modelos: inventário de GGUFs em disco.

use eframe::egui;
use orchestrator_studio::models::ModelsState;

/// Diretório, botão de inventário, erro e tabela de artefatos.
pub fn show(ui: &mut egui::Ui, models: &mut ModelsState) {
    ui.collapsing("Modelos (arquivos GGUF em disco)", |ui| {
        let mut dir_text = models.dir.display().to_string();
        ui.horizontal(|ui| {
            ui.label("diretório:");
            if ui.text_edit_singleline(&mut dir_text).changed() {
                models.dir = dir_text.into();
            }
            if ui.button("Inventariar").clicked() {
                models.refresh();
            }
        });
        if !models.error.is_empty() {
            ui.label(&models.error);
        }
        if models.list.is_empty() {
            ui.label("Nenhum GGUF inventariado. Clique Inventariar.");
        } else {
            egui::Grid::new("models_grid").show(ui, |ui| {
                ui.label("arquivo");
                ui.label("tamanho");
                ui.label("sha-256");
                ui.end_row();
                for artifact in &models.list {
                    ui.label(&artifact.file_name);
                    ui.label(format!("{} bytes", artifact.size_bytes));
                    ui.label(artifact.sha256_hex.chars().take(16).collect::<String>());
                    ui.end_row();
                }
            });
        }
    });
}
