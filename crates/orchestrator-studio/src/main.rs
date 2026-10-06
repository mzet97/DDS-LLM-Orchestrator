//! Casca eframe do Studio: navegação lateral + painel central (§30 SDD).
//!
//! Sem lógica de domínio aqui — cada painel lê capacidade real e o estado
//! testável vive nos módulos da lib.

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::catalog_remote::SharedCatalog;
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::launch::LaunchState;
use orchestrator_studio::machines::MachinesState;
use orchestrator_studio::models::ModelsState;
use orchestrator_studio::services::ServicesPanel;
use orchestrator_studio::state::AppState;
use orchestrator_studio::workload::DispatchState;

/// Seção exibida no painel central (navegação lateral exigida no §30).
/// Sem "Catálogo" local (T-830-04): a leitura do catálogo vive em
/// "Catálogo compartilhado" (autoridade no nó via `catalog_remote`).
/// "Máquinas" (REQ/T-840-03) registra nós remotos no MESMO catálogo
/// compartilhado e sonda cada um via `GET /version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    Node,
    Inference,
    Launch,
    Agents,
    Dispatch,
    Models,
    Services,
    Shared,
    Machines,
    Topology,
    Workflow,
    #[cfg(feature = "dds")]
    Tools,
    Logs,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Visão geral",
            Self::Node => "Nó studio-node",
            Self::Inference => "Inferência",
            Self::Launch => "Subir inferência",
            Self::Agents => "Agentes",
            Self::Dispatch => "Despacho",
            Self::Models => "Modelos GGUF",
            Self::Services => "Serviços",
            Self::Shared => "Catálogo compartilhado",
            Self::Machines => "Máquinas",
            Self::Topology => "Topologia DDS",
            Self::Workflow => "Workflow (A→B→C)",
            #[cfg(feature = "dds")]
            Self::Tools => "Ferramentas",
            Self::Logs => "Logs",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Overview,
            Self::Node,
            Self::Inference,
            Self::Launch,
            Self::Agents,
            Self::Dispatch,
            Self::Models,
            Self::Services,
            Self::Shared,
            Self::Machines,
            Self::Topology,
            Self::Workflow,
            #[cfg(feature = "dds")]
            Self::Tools,
            Self::Logs,
        ]
    }
}

struct StudioApp {
    section: Section,
    state: AppState,
    node_url: String,
    /// Token do nó do painel "Nó studio-node" (T-840-03a) — só memória.
    node_token: String,
    inference: InferenceState,
    launch: LaunchState,
    agents: AgentsState,
    models: ModelsState,
    services: ServicesPanel,
    shared: SharedCatalog,
    machines: MachinesState,
    dispatch: DispatchState,
    #[cfg(feature = "dds")]
    dds: orchestrator_studio::dds_observe::DdsState,
    workflow: orchestrator_studio::workflow::WorkflowState,
    protected: orchestrator_studio::protected::ProtectedGuard,
    discovery: orchestrator_studio::discovery::DiscoveryState,
    /// Alvo já carregado automaticamente (auto-carga única por troca de alvo).
    autoloaded_target: Option<String>,
    /// Observação DDS inicial já disparada (Topologia/Ferramentas ao abrir).
    #[cfg(feature = "dds")]
    dds_observed: bool,
}

impl StudioApp {
    fn new() -> Self {
        Self {
            section: Section::Overview,
            state: AppState::new(),
            node_url: String::from("http://127.0.0.1:4317"),
            node_token: String::new(),
            inference: InferenceState::new(),
            launch: LaunchState::new(),
            agents: AgentsState::new(),
            models: ModelsState::new(),
            services: ServicesPanel::new(),
            shared: SharedCatalog::with_url("http://127.0.0.1:4317"),
            machines: MachinesState::with_url("http://127.0.0.1:4317"),
            dispatch: DispatchState::new(),
            #[cfg(feature = "dds")]
            dds: orchestrator_studio::dds_observe::DdsState::new(),
            workflow: orchestrator_studio::workflow::WorkflowState::new(),
            protected: orchestrator_studio::protected::ProtectedGuard::new(),
            // T-890-03: descoberta automática no boot — escuta
            // Studio.NodePresence em background (domínio: env
            // STUDIO_DDS_DOMAIN, default 170 = laboratório).
            autoloaded_target: None,
            #[cfg(feature = "dds")]
            dds_observed: false,
            discovery: {
                let domain = std::env::var("STUDIO_DDS_DOMAIN")
                    .ok()
                    .and_then(|d| d.trim().parse().ok())
                    .unwrap_or(170);
                #[cfg(feature = "dds")]
                {
                    orchestrator_studio::discovery::DiscoveryState::start(domain)
                }
                #[cfg(not(feature = "dds"))]
                orchestrator_studio::discovery::DiscoveryState::new_disabled(domain)
            },
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // T-890-03: descoberta → alvo único. O nó selecionado (auto: primeiro
        // online; manual: combobox no painel Máquinas) propaga para TODOS os
        // painéis que falam com um studio-node — trocar uma vez, muda tudo.
        self.discovery.poll();
        if self.discovery.autoselect_first_online() {
            orchestrator_studio::studio_log::info(format!(
                "alvo automático: {}",
                self.discovery.selected_url().unwrap_or_default()
            ));
        }
        if let Some(target) = self.discovery.selected_url() {
            self.node_url = target.clone();
            self.services.url = target.clone();
            self.shared.url = target.clone();
            self.machines.url = target.clone();
            // AUTO-CARGA (T-890-03): trocou o alvo → os painéis disparam as
            // próprias leituras SOZINHOS (Nó conecta, Serviços lê o plano,
            // Catálogo tira o snapshot). Token do deploy quando existir.
            if self.autoloaded_target.as_deref() != Some(target.as_str()) {
                self.autoloaded_target = Some(target.clone());
                let token = orchestrator_studio::discovery::token_for_url(
                    &target,
                    std::env::var("HOME").ok().as_deref(),
                );
                if let Some(token) = &token {
                    self.node_token = token.clone();
                }
                orchestrator_studio::studio_log::info(format!(
                    "auto-carga do alvo {target}: Nó (token={}) + Serviços + Catálogo",
                    if token.is_some() { "sim" } else { "não" }
                ));
                self.state
                    .refresh_from_node_with_token(&target, token.as_deref());
                self.services.refresh();
                self.shared.refresh();
            }
        }

        // Topologia/Ferramentas: primeira observação automática ao entrar na
        // aba (domínio default já é o da descoberta — ver DdsState::new).
        #[cfg(feature = "dds")]
        if matches!(self.section, Section::Topology | Section::Tools)
            && !self.dds_observed
            && !self.dds.busy
        {
            self.dds_observed = true;
            orchestrator_studio::studio_log::info(format!(
                "topologia: observação automática do domínio {}",
                self.dds.domain
            ));
            self.dds.refresh();
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(self.state.status());
            ui.separator();
            ui.label(format!(
                "🛰 descoberta: {} nó(s) · {} agente(s) · {} inferência(s)",
                self.discovery.nodes.len(),
                self.discovery.agents.len(),
                self.discovery.servers.len()
            ));
            if let Some(target) = self.discovery.selected_url() {
                ui.separator();
                ui.label(format!("🛰 alvo: {target}"));
            }
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
            ui.separator();
            // T-890-08 (G-38/65): modo protegido — ações com efeito real.
            let armed = self.protected.armed;
            let guard_label = self.protected.label();
            if ui.selectable_label(armed, guard_label).clicked() {
                self.protected.set_armed(!armed);
            }
            if let Some(refusal) = &self.protected.last_refusal {
                ui.colored_label(egui::Color32::RED, refusal);
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
                    orchestrator_studio::views::overview::show(
                        ui,
                        &self.state,
                        &self.services,
                        &self.agents,
                        &self.models,
                        proof,
                    );
                }
                Section::Node => {
                    orchestrator_studio::views::node::show(
                        ui,
                        &mut self.state,
                        &mut self.node_url,
                        &mut self.node_token,
                    );
                }
                Section::Inference => orchestrator_studio::views::inference::show(
                    ui,
                    &mut self.inference,
                    &self.discovery,
                ),
                Section::Launch => {
                    let known: Vec<String> = self
                        .services
                        .list
                        .iter()
                        .map(|item| item.service.clone())
                        .collect();
                    orchestrator_studio::views::launch::show(ui, &mut self.launch, &known);
                }
                Section::Agents => {
                    orchestrator_studio::views::agents::show(ui, &mut self.agents, &self.discovery)
                }
                Section::Models => orchestrator_studio::views::models::show(ui, &mut self.models),
                Section::Services => orchestrator_studio::views::services::show(
                    ui,
                    &mut self.services,
                    &mut self.protected,
                ),
                Section::Shared => {
                    orchestrator_studio::views::shared_catalog::show(ui, &mut self.shared)
                }
                Section::Machines => orchestrator_studio::views::machines::show(
                    ui,
                    &mut self.machines,
                    &mut self.discovery,
                ),
                Section::Dispatch => {
                    orchestrator_studio::views::dispatch::show(ui, &mut self.dispatch)
                }
                #[cfg(feature = "dds")]
                Section::Topology => orchestrator_studio::views::topology::show(ui, &mut self.dds),
                Section::Workflow => {
                    orchestrator_studio::views::workflow::show(ui, &mut self.workflow)
                }
                #[cfg(feature = "dds")]
                Section::Tools => orchestrator_studio::views::tools::show(ui, &mut self.dds),
                Section::Logs => orchestrator_studio::views::logs::show(ui),
                #[cfg(not(feature = "dds"))]
                Section::Topology => {
                    ui.heading("Topologia DDS");
                    ui.label(
                        "Recompile com --features dds para observar Tasks/TaskOutput ao vivo.",
                    );
                }
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
