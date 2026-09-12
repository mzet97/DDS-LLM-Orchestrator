//! Casca eframe do Studio: navegação lateral + painel central (§30 SDD).
//!
//! Sem lógica de domínio aqui — cada painel lê capacidade real e o estado
//! testável vive nos módulos da lib.

mod views;

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::agent_defs::DefinitionStore;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::catalog_remote::SharedCatalog;
use orchestrator_studio::design::{self, Theme};
use orchestrator_studio::gallery::GalleryState;
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::launch::LaunchState;
use orchestrator_studio::machines::MachineLedger;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::nodes::NodeRegistry;
use orchestrator_studio::orchestrators::OrchestratorPanel;
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::shell::{CommandItem, ShellContext};
use orchestrator_studio::ssh_session::SshSession;
use orchestrator_studio::state::AppState;
use orchestrator_studio::workload::DispatchState;
use studio_core::catalog::Catalog;

/// Seção exibida no painel central (navegação lateral exigida no §30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    Environments,
    Machines,
    Orchestrators,
    Catalog,
    Node,
    Inference,
    Launch,
    Ssh,
    Agents,
    Dispatch,
    Models,
    Services,
    Shared,
    Topology,
    Gallery,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Visão geral",
            Self::Environments => "Ambientes",
            Self::Machines => "Máquinas e rede",
            Self::Orchestrators => "Orquestradores",
            Self::Catalog => "Catálogo",
            Self::Node => "Nó studio-node",
            Self::Inference => "Inferência",
            Self::Launch => "Subir inferência",
            Self::Ssh => "SSH dedicado",
            Self::Agents => "Agentes",
            Self::Dispatch => "Despacho",
            Self::Models => "Modelos GGUF",
            Self::Services => "Serviços",
            Self::Shared => "Catálogo compartilhado",
            Self::Topology => "Topologia DDS",
            Self::Gallery => "Galeria",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Overview,
            Self::Environments,
            Self::Machines,
            Self::Orchestrators,
            Self::Catalog,
            Self::Node,
            Self::Inference,
            Self::Launch,
            Self::Ssh,
            Self::Agents,
            Self::Dispatch,
            Self::Models,
            Self::Services,
            Self::Shared,
            Self::Topology,
            Self::Gallery,
        ]
    }

    fn command_entries() -> Vec<CommandItem> {
        Self::all()
            .iter()
            .map(|section| CommandItem {
                title: format!("Ir para {}", section.label()),
                section: String::from("Navegação"),
                dangerous: false,
            })
            .collect()
    }
}

struct StudioApp {
    section: Section,
    shell: ShellContext,
    theme: Theme,
    palette_open: bool,
    palette_query: String,
    gallery: GalleryState,
    state: AppState,
    catalog: Catalog,
    registry: NodeRegistry,
    inference: InferenceState,
    launch: LaunchState,
    ssh: SshSession,
    agents: AgentsState,
    agents_tab: views::agents::AgentsTab,
    definitions: DefinitionStore,
    machines: MachineLedger,
    orchestrators: OrchestratorPanel,
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
            shell: ShellContext::default(),
            theme: Theme::default(),
            palette_open: false,
            palette_query: String::new(),
            gallery: GalleryState::new(design::ResolvedTheme::Light),
            state: AppState::new(),
            catalog: Catalog::new(),
            registry: NodeRegistry::new(),
            inference: InferenceState::new(),
            launch: LaunchState::new(),
            ssh: SshSession::new(),
            agents: AgentsState::new(),
            agents_tab: views::agents::AgentsTab::default(),
            definitions: DefinitionStore::new(),
            machines: MachineLedger::new(),
            orchestrators: OrchestratorPanel::new(),
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
        let resolved = self.theme.resolve(None);
        design::apply(ui.ctx(), resolved);
        self.gallery.theme = resolved;
        self.shell.theme = self.theme;
        if ui.input(|input| input.key_pressed(egui::Key::K) && input.modifiers.command) {
            self.palette_open = true;
        }
        egui::Panel::top("topbar").show(ui, |ui| {
            orchestrator_studio::shell::show_top_bar(ui, &mut self.shell, &mut self.palette_open);
        });
        if self.palette_open {
            let entries = Section::command_entries();
            if let Some(picked) = orchestrator_studio::shell::show_palette(
                ui,
                &mut self.palette_open,
                &mut self.palette_query,
                &entries,
            ) {
                if let Some(title) = picked.title.strip_prefix("Ir para ") {
                    if let Some(section) = Section::all().iter().find(|item| item.label() == title)
                    {
                        self.section = *section;
                    }
                }
            }
        }
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
                Section::Node => views::node::show(ui, &mut self.state, &mut self.registry),
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
                Section::Ssh => views::ssh::show(ui, &mut self.ssh, &self.registry),
                Section::Environments => {
                    views::environments::show(ui, &self.registry, &mut self.shell);
                }
                Section::Machines => views::machines::show(ui, &mut self.machines),
                Section::Orchestrators => {
                    views::orchestrators::show(ui, &mut self.orchestrators, &self.agents);
                }
                Section::Agents => {
                    views::agents::show(
                        ui,
                        &mut self.agents,
                        &mut self.definitions,
                        &mut self.agents_tab,
                    );
                }
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
                Section::Gallery => orchestrator_studio::gallery::show(ui, &self.gallery),
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
