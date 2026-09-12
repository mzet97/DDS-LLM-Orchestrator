//! Casca eframe do Studio: navegação lateral + painel central (§30 SDD).
//!
//! Sem lógica de domínio aqui — cada painel lê capacidade real e o estado
//! testável vive nos módulos da lib.

mod views;

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::catalog_remote::SharedCatalog;
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::launch::LaunchState;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::state::AppState;
use orchestrator_studio::workload::DispatchState;
use studio_core::catalog::Catalog;

/// Seção exibida no painel central (navegação lateral exigida no §30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    Catalog,
    Node,
    Inference,
    Launch,
    Agents,
    Dispatch,
    Models,
    Services,
    Shared,
    Topology,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Visão geral",
            Self::Catalog => "Catálogo",
            Self::Node => "Nó studio-node",
            Self::Inference => "Inferência",
            Self::Launch => "Subir inferência",
            Self::Agents => "Agentes",
            Self::Dispatch => "Despacho",
            Self::Models => "Modelos GGUF",
            Self::Services => "Serviços",
            Self::Shared => "Catálogo compartilhado",
            Self::Topology => "Topologia DDS",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Overview,
            Self::Catalog,
            Self::Node,
            Self::Inference,
            Self::Launch,
            Self::Agents,
            Self::Dispatch,
            Self::Models,
            Self::Services,
            Self::Shared,
            Self::Topology,
        ]
    }
}

struct StudioApp {
    section: Section,
    state: AppState,
    catalog: Catalog,
    node_url: String,
    inference: InferenceState,
    launch: LaunchState,
    agents: AgentsState,
    models: ModelsState,
    services: ServicesPanel,
    shared: SharedCatalog,
    dispatch: DispatchState,
    #[cfg(feature = "dds")]
    dds: orchestrator_studio::dds_observe::DdsState,
}

impl StudioApp {
    fn new() -> Self {
        Self {
            section: Section::Overview,
            state: AppState::new(),
            catalog: Catalog::new(),
            node_url: String::from("http://127.0.0.1:4317"),
            inference: InferenceState::new(),
            launch: LaunchState::new(),
            agents: AgentsState::new(),
            models: ModelsState::new(),
            services: ServicesPanel::new(),
            shared: SharedCatalog::with_url("http://127.0.0.1:4317"),
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
        egui::Panel::left("nav").show(ui, |ui| {
            ui.heading("Studio");
            for section in Section::all() {
                let busy = matches!(section, Section::Models) && self.models.is_busy();
                let label = if busy {
                    format!("{} …", section.label())
                } else {
                    section.label().to_string()
                };
                if ui
                    .selectable_label(self.section == *section, label)
                    .clicked()
                {
                    self.section = *section;
                }
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.section {
                Section::Overview => {
                    let proof = if self.launch.proved() {
                        self.launch
                            .steps
                            .last()
                            .map(|step| step.detail.as_str())
                            .unwrap_or("")
                    } else {
                        ""
                    };
                    views::overview::show(
                        ui,
                        &self.state,
                        &self.services,
                        &self.agents,
                        &self.models,
                        proof,
                    );
                }
                Section::Node => views::node::show(ui, &mut self.state, &mut self.node_url),
                Section::Inference => views::inference::show(ui, &mut self.inference),
                Section::Launch => {
                    let known: Vec<String> = self
                        .services
                        .list
                        .iter()
                        .map(|item| item.service.clone())
                        .collect();
                    views::launch::show(ui, &mut self.launch, &known);
                }
                Section::Agents => views::agents::show(ui, &mut self.agents),
                Section::Models => views::models::show(ui, &mut self.models),
                Section::Services => views::services::show(ui, &mut self.services),
                Section::Shared => views::shared_catalog::show(ui, &mut self.shared),
                Section::Dispatch => views::dispatch::show(ui, &mut self.dispatch),
                #[cfg(feature = "dds")]
                Section::Topology => views::topology::show(ui, &mut self.dds),
                #[cfg(not(feature = "dds"))]
                Section::Topology => {
                    ui.heading("Topologia DDS");
                    ui.label(
                        "Recompile com --features dds para observar Tasks/TaskOutput ao vivo.",
                    );
                }
                Section::Catalog => views::catalog::show(ui, &mut self.catalog, &mut self.state),
            });
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
