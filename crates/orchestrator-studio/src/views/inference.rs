//! Painel de inferência: sessão multi-turn contra o llama ao vivo.

use eframe::egui;
use orchestrator_studio::inference::{InferenceState, Role};

/// Servidor, modelos, parâmetros, prompt e transcript da sessão.
pub fn show(ui: &mut egui::Ui, infer: &mut InferenceState) {
    ui.collapsing("Inferência (servidor llama ao vivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("servidor:");
            ui.text_edit_singleline(&mut infer.server_url);
            if ui.button("Modelos").clicked() {
                infer.refresh_models();
            }
        });
        if infer.models.is_empty() {
            ui.text_edit_singleline(&mut infer.model);
        } else {
            egui::ComboBox::from_label("modelo")
                .selected_text(&infer.model)
                .show_ui(ui, |ui| {
                    for candidate in infer.models.clone() {
                        ui.selectable_value(&mut infer.model, candidate.clone(), candidate);
                    }
                });
        }
        ui.add(egui::Slider::new(&mut infer.temperature, 0.0..=2.0).text("temperatura"));
        ui.add(egui::Slider::new(&mut infer.max_tokens, 1..=4096).text("máx. tokens"));
        ui.label("prompt:");
        ui.text_edit_multiline(&mut infer.prompt);
        ui.horizontal(|ui| {
            if ui.button("Enviar").clicked() {
                infer.send();
            }
            if ui.button("Nova sessão").clicked() {
                infer.clear_session();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                if infer.history.is_empty() {
                    ui.label(&infer.reply);
                }
                for message in &infer.history {
                    let who = match message.role {
                        Role::System => "sistema",
                        Role::User => "você",
                        Role::Assistant => "assistente",
                    };
                    ui.label(format!("{who}: {}", message.content));
                }
            });
    });
}
