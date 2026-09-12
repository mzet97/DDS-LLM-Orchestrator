//! Casca eframe do Studio: orquestra os painéis de `views`.
//!
//! Sem lógica de domínio aqui — cada painel lê capacidade real e o estado
//! testável vive nos módulos da lib.

mod views;

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::state::AppState;
use orchestrator_studio::workload::DispatchState;
use studio_core::catalog::Catalog;

struct StudioApp {
    state: AppState,
    catalog: Catalog,
    node_url: String,
    inference: InferenceState,
    agents: AgentsState,
    models: ModelsState,
    services: ServicesPanel,
    dispatch: DispatchState,
    #[cfg(feature = "dds")]
    dds: orchestrator_studio::dds_observe::DdsState,
}

impl StudioApp {
    fn new() -> Self {
        Self {
            state: AppState::new(),
            catalog: Catalog::new(),
            node_url: String::from("http://127.0.0.1:4317"),
            inference: InferenceState::new(),
            agents: AgentsState::new(),
            models: ModelsState::new(),
            services: ServicesPanel::new(),
            dispatch: DispatchState::new(),
            #[cfg(feature = "dds")]
            dds: orchestrator_studio::dds_observe::DdsState::new(),
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
            views::node::show(ui, &mut self.state, &mut self.node_url);
            views::inference::show(ui, &mut self.inference);
            views::agents::show(ui, &mut self.agents);
            views::models::show(ui, &mut self.models);
            views::services::show(ui, &mut self.services);
            views::dispatch::show(ui, &mut self.dispatch);
            #[cfg(feature = "dds")]
            views::topology::show(ui, &mut self.dds);
            views::catalog::show(ui, &mut self.catalog, &mut self.state);
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
