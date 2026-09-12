//! Casca eframe do Studio: renderiza [`AppState`] sem inventar dados.
//!
//! Começa vazia; "Atualizar" relê o catálogo em memória e "Conectar ao nó"
//! busca o resumo vivo do `studio-noded` (T-800-08).

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::state::AppState;
use studio_core::catalog::Catalog;

struct StudioApp {
    state: AppState,
    catalog: Catalog,
    node_url: String,
    inference: InferenceState,
}

impl StudioApp {
    fn new() -> Self {
        Self {
            state: AppState::new(),
            catalog: Catalog::new(),
            node_url: String::from("http://127.0.0.1:4317"),
            inference: InferenceState::new(),
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(self.state.status());
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("DDS Orchestrator Studio");
            ui.horizontal(|ui| {
                ui.label("nó:");
                ui.text_edit_singleline(&mut self.node_url);
                if ui.button("Conectar ao nó").clicked() {
                    self.state.refresh_from_node(&self.node_url.clone());
                }
                if ui.button("Atualizar").clicked() {
                    let snapshot = self.catalog.snapshot();
                    self.state.refresh_from(&snapshot);
                }
            });
            if let Some(node) = self.state.node() {
                ui.separator();
                ui.label(format!(
                    "nó: protocolo {}.{} · {} operação(ões)",
                    node.version.major,
                    node.version.minor,
                    node.operations.len()
                ));
            }
            ui.collapsing("Inferência (servidor llama ao vivo)", |ui| {
                let infer = &mut self.inference;
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
                if ui.button("Enviar").clicked() {
                    infer.send();
                }
                ui.separator();
                ui.label(&infer.reply);
            });
            if self.state.rows().is_empty() {
                ui.label("Nenhum item no catálogo. Use \"Conectar ao nó\" para ler a origem viva.");
            } else {
                egui::Grid::new("catalog_grid").show(ui, |ui| {
                    ui.label("id");
                    ui.label("valor");
                    ui.label("revisão");
                    ui.end_row();
                    for row in self.state.rows() {
                        ui.label(&row.id);
                        ui.label(&row.value);
                        ui.label(row.revision.to_string());
                        ui.end_row();
                    }
                });
            }
        });
    }
}

fn main() -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "DDS Orchestrator Studio",
        options,
        Box::new(|_cc| Ok(Box::new(StudioApp::new()))),
    )
    .map_err(|err| anyhow::anyhow!("falha ao abrir a janela: {err}"))
}
