//! Casca eframe do Studio: renderiza [`AppState`] sem inventar dados.
//!
//! Começa vazia (nenhum catálogo carregado); o botão "Atualizar" relê o
//! catálogo em memória nesta fase — a origem remota chega em T-800-05
//! com o protocolo versionado do studio-node.

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::state::AppState;
use studio_core::catalog::Catalog;

struct StudioApp {
    state: AppState,
    catalog: Catalog,
}

impl StudioApp {
    fn new() -> Self {
        Self {
            state: AppState::new(),
            catalog: Catalog::new(),
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
            if ui.button("Atualizar").clicked() {
                let snapshot = self.catalog.snapshot();
                self.state.refresh_from(&snapshot);
            }
            if self.state.rows().is_empty() {
                ui.label("Nenhum item no catálogo. Conecte uma origem (T-800-05).");
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
