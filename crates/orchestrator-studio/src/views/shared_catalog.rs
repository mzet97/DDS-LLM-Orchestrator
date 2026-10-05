//! Painel de catálogo compartilhado: publica no nó com base explícita.
//! Token da autoridade (T-840-03a) fica só na memória da GUI.

use eframe::egui;
use orchestrator_studio::catalog_remote::SharedCatalog;

/// URL, token (password), formulário (id/valor/base), publicar/excluir e
/// tabela do snapshot.
pub fn show(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    shared.poll();
    ui.collapsing("Catálogo compartilhado (autoridade no nó)", |ui| {
        ui.horizontal(|ui| {
            ui.label("nó:");
            ui.text_edit_singleline(&mut shared.url);
            ui.label("token:");
            ui.add(egui::TextEdit::singleline(&mut shared.token).password(true));
            ui.add_enabled_ui(!shared.busy, |ui| {
                if ui.button("Ler snapshot").clicked() {
                    shared.refresh();
                }
                // Acompanhamento incremental desde o cursor (T-830-02).
                if ui.button("Acompanhar eventos").clicked() {
                    shared.follow_events();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("id:");
            ui.text_edit_singleline(&mut shared.form_id);
            ui.label("valor:");
            ui.text_edit_singleline(&mut shared.form_value);
            ui.label("base:");
            ui.text_edit_singleline(&mut shared.form_base);
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!shared.busy, |ui| {
                if ui.button("Publicar").clicked() {
                    shared.publish_form();
                }
                if ui.button("Excluir").clicked() {
                    shared.delete_form();
                }
            });
        });
        if !shared.notice.is_empty() {
            ui.label(&shared.notice);
        }
        match &shared.snapshot {
            None => {
                ui.label("Sem snapshot. Clique Ler snapshot.");
            }
            Some(snapshot) if snapshot.items.is_empty() => {
                ui.label(format!("Catálogo vazio (cursor {}).", snapshot.cursor.0));
            }
            Some(snapshot) => {
                egui::Grid::new("shared_catalog_grid").show(ui, |ui| {
                    ui.label("id");
                    ui.label("valor");
                    ui.label("revisão");
                    ui.end_row();
                    for item in &snapshot.items {
                        ui.label(&item.id.0);
                        ui.label(&item.value);
                        ui.label(item.revision.0.to_string());
                        ui.end_row();
                    }
                });
                ui.label(format!("cursor: {}", snapshot.cursor.0));
            }
        }
    });
}
