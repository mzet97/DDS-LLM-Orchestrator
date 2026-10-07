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
/// Grupos da navegação (migração Stitch: sidebar em 5 grupos, v6).
const NAV_GROUPS: &[(&str, &[Section])] = &[
    (
        "Sistema",
        &[Section::Overview, Section::Topology, Section::Logs],
    ),
    (
        "Nós & Infraestrutura",
        &[
            Section::Machines,
            Section::Node,
            Section::Services,
            Section::Shared,
        ],
    ),
    (
        "Enxame IA & Inferência",
        &[
            Section::Agents,
            Section::Inference,
            Section::Launch,
            Section::Workflow,
            #[cfg(feature = "dds")]
            Section::Tools,
        ],
    ),
    ("Artefatos", &[Section::Models]),
    ("Diagnóstico", &[Section::Dispatch]),
];

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

    /// Numeração do mockup (sidebar "3.1 … 3.14").
    fn num(self) -> &'static str {
        match self {
            Self::Overview => "3.1",
            Self::Node => "3.2",
            Self::Inference => "3.3",
            Self::Launch => "3.4",
            Self::Agents => "3.5",
            Self::Dispatch => "3.6",
            Self::Models => "3.7",
            Self::Services => "3.8",
            Self::Shared => "3.9",
            Self::Machines => "3.10",
            Self::Topology => "3.11",
            Self::Workflow => "3.12",
            #[cfg(feature = "dds")]
            Self::Tools => "3.13",
            Self::Logs => "3.14",
        }
    }

    /// Tag curta mono (badge da sidebar nos mockups: DOM/MESH/HB/…).
    fn tag(self) -> &'static str {
        match self {
            Self::Overview => "DOM",
            Self::Node => "RPC",
            Self::Inference => "LLAMA",
            Self::Launch => "RUN",
            Self::Agents => "HB",
            Self::Dispatch => ":8080",
            Self::Models => "SHA",
            Self::Services => "SYSTEMD",
            Self::Shared => "REV",
            Self::Machines => "DDS",
            Self::Topology => "MESH",
            Self::Workflow => "DAG",
            #[cfg(feature = "dds")]
            Self::Tools => "SEC",
            Self::Logs => "500",
        }
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
    /// Auto-carga contínua do alvo a cada 5 s (toggle do header, mockups).
    auto_reload: bool,
    /// Instante da última auto-carga (None = nenhuma ainda).
    last_autoload: Option<std::time::Instant>,
}

/// Hash curto do git da árvore (build.rs; "dev" fora de repositório).
const GIT_HASH: &str = match option_env!("STUDIO_GIT_HASH") {
    Some(hash) => hash,
    None => "dev",
};

impl StudioApp {
    fn new() -> Self {
        Self {
            // Tela inicial = Topologia DDS (decisão do autor 2026-10-06): o
            // mapa vivo do domínio é a porta de entrada; sem a feature `dds`
            // o fallback honesto é a Visão geral.
            section: {
                #[cfg(feature = "dds")]
                {
                    Section::Topology
                }
                #[cfg(not(feature = "dds"))]
                {
                    Section::Overview
                }
            },
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
            auto_reload: true,
            last_autoload: None,
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
        // Header 40px (migração Stitch): título + badge de domínio + chip de
        // alvo + modo protegido. O repaint pulsante do dot é do ctx.
        egui::Panel::top("header")
            .exact_size(40.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.strong("DDS Orchestrator Studio");
                    ui.label(
                        egui::RichText::new(format!("v1.0-{GIT_HASH}"))
                            .monospace()
                            .small()
                            .color(orchestrator_studio::theme::OUTLINE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // chip de modo protegido (mockup: último à direita)
                        let armed = self.protected.armed;
                        let chip_text = if armed {
                            "🛡 MODO PROTEGIDO: ARMADO (clique p/ desarmar)"
                        } else {
                            "🛡 MODO PROTEGIDO: DESARMADO (clique p/ armar)"
                        };
                        let chip_color = if armed {
                            orchestrator_studio::theme::ERROR
                        } else {
                            orchestrator_studio::theme::ON_SURFACE_VARIANT
                        };
                        let chip = ui.add(egui::Button::new(
                            egui::RichText::new(chip_text)
                                .monospace()
                                .small()
                                .color(chip_color),
                        ));
                        if chip.clicked() {
                            self.protected.set_armed(!armed);
                        }
                        if let Some(refusal) = &self.protected.last_refusal {
                            if armed {
                                ui.label(
                                    egui::RichText::new(refusal)
                                        .small()
                                        .color(orchestrator_studio::theme::ERROR),
                                );
                            }
                        }
                        // toggle Auto-Carga (5s): re-leituras periódicas do alvo
                        let auto = ui.selectable_label(
                            self.auto_reload,
                            egui::RichText::new("Auto-Carga (5s)")
                                .monospace()
                                .small()
                                .color(if self.auto_reload {
                                    orchestrator_studio::theme::PRIMARY_FIXED_DIM
                                } else {
                                    orchestrator_studio::theme::STALE
                                }),
                        );
                        if auto.clicked() {
                            self.auto_reload = !self.auto_reload;
                            orchestrator_studio::studio_log::info(format!(
                                "auto-carga contínua: {}",
                                if self.auto_reload { "ligada" } else { "desligada" }
                            ));
                        }
                        // chip NÓ ALVO (3.1/3.10): URL + estado de conexão
                        if let Some(target) = self.discovery.selected_url() {
                            let connected = self.state.node().is_some();
                            let state_text = if connected {
                                format!(
                                    "Conectado v{}.{}",
                                    self.state.node().map(|n| n.version.major).unwrap_or(1),
                                    self.state.node().map(|n| n.version.minor).unwrap_or(0)
                                )
                            } else {
                                String::from("conectando…")
                            };
                            let (rect, _) =
                                ui.allocate_at_least(egui::vec2(280.0, 22.0), egui::Sense::hover());
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(
                                    orchestrator_studio::theme::RADIUS_SM as u8,
                                ),
                                orchestrator_studio::theme::tint(
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                    10,
                                ),
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 8.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("NÓ ALVO: {target}"),
                                egui::FontId::monospace(10.0),
                                orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                            );
                            ui.painter().text(
                                egui::pos2(rect.right() - 8.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                state_text,
                                egui::FontId::monospace(9.0),
                                if connected {
                                    orchestrator_studio::theme::OK
                                } else {
                                    orchestrator_studio::theme::WARN
                                },
                            );
                        }
                        // badge de domínio com dot pulsante
                        let (rect, _) =
                            ui.allocate_at_least(egui::vec2(140.0, 22.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(orchestrator_studio::theme::RADIUS_SM as u8),
                            orchestrator_studio::theme::SURFACE_HIGH,
                        );
                        let any_alive = self
                            .discovery
                            .nodes
                            .iter()
                            .any(|n| n.is_alive(orchestrator_studio::machines::now_unix_ns()));
                        let pulse: f32 = if any_alive {
                            (ui.input(|i| i.time) * 2.0).sin() as f32 * 0.5 + 0.5
                        } else {
                            1.0
                        };
                        ui.painter().circle_filled(
                            egui::pos2(rect.left() + 8.0, rect.center().y),
                            3.0,
                            orchestrator_studio::theme::PRIMARY_CONTAINER
                                .gamma_multiply(1.0 - pulse * 0.5),
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 16.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("DDS DOMAIN {}", self.discovery.domain),
                            egui::FontId::monospace(10.0),
                            orchestrator_studio::theme::ON_SURFACE_VARIANT,
                        );
                    });
                });
            });

        // T-890-03: descoberta → alvo único. O nó selecionado (auto: primeiro
        // online; manual: combobox no painel Máquinas) propaga para TODOS os
        // painéis que falam com um studio-node — trocar uma vez, muda tudo.
        self.discovery.poll();
        // Drena a leitura do nó alvo em QUALQUER tela (o header e a barra de
        // status dependem dela — antes só a view Nó drenava).
        self.state.poll();
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

        // Auto-carga contínua (toggle do header, mockup "Auto-Carga (5s)"):
        // re-leituras periódicas do alvo — cada refresh é no-op enquanto busy.
        if self.auto_reload {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
            if self
                .last_autoload
                .is_none_or(|at| at.elapsed().as_secs() >= 5)
            {
                self.last_autoload = Some(std::time::Instant::now());
                if let Some(target) = self.discovery.selected_url() {
                    let token = orchestrator_studio::discovery::token_for_url(
                        &target,
                        std::env::var("HOME").ok().as_deref(),
                    );
                    self.state
                        .refresh_from_node_with_token(&target, token.as_deref());
                    self.services.refresh();
                    self.shared.refresh();
                }
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
        egui::Panel::bottom("status")
            .exact_size(52.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let mono = |text: String| {
                    egui::RichText::new(text)
                        .monospace()
                        .small()
                        .color(orchestrator_studio::theme::ON_SURFACE_VARIANT)
                };
                ui.label(mono(format!(
                    "ESTADO DO NÓ: {}",
                    self.state.status()
                )));
                ui.label(mono(format!(
                    "DESCOBERTA DDS: {} nó(s) ativos (lease 10s) · {} agente(s) · {} servidor(es) de inferência | Domínio: {}",
                    self.discovery.nodes.len(),
                    self.discovery.agents.len(),
                    self.discovery.servers.len(),
                    self.discovery.domain
                )));
                let alvo = self
                    .discovery
                    .selected_url()
                    .unwrap_or_else(|| String::from("— nenhum alvo —"));
                ui.label(mono(format!(
                    "ALVO SELECIONADO: {alvo} | Token: apenas em memória, sem segredos em disco"
                )));
            });
        egui::Panel::left("nav")
            .exact_size(256.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(orchestrator_studio::theme::SPACE_SM);
                for (group, sections) in NAV_GROUPS {
                    ui.label(
                        egui::RichText::new((*group).to_uppercase())
                            .monospace()
                            .small()
                            .color(orchestrator_studio::theme::OUTLINE),
                    );
                    ui.add_space(orchestrator_studio::theme::SPACE_XS);
                    for section in *sections {
                        let busy = matches!(section, Section::Models) && self.models.is_busy();
                        let label = if busy {
                            format!("{} {}", section.num(), {
                                let base = section.label();
                                format!("{base} …")
                            })
                        } else {
                            format!("{} {}", section.num(), section.label())
                        };
                        let selected = self.section == *section;
                        let text = if selected {
                            egui::RichText::new(&label)
                                .strong()
                                .color(orchestrator_studio::theme::PRIMARY_CONTAINER)
                        } else {
                            egui::RichText::new(&label)
                                .color(orchestrator_studio::theme::ON_SURFACE)
                        };
                        ui.horizontal(|ui| {
                            let response = ui
                                .selectable_label(selected, text)
                                .interact(egui::Sense::click());
                            if response.clicked() {
                                self.section = *section;
                            }
                            ui.label(
                                egui::RichText::new(section.tag())
                                    .monospace()
                                    .small()
                                    .color(orchestrator_studio::theme::OUTLINE),
                            );
                        });
                    }
                    ui.add_space(orchestrator_studio::theme::SPACE_MD);
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
                        &mut self.state,
                        &mut self.services,
                        &mut self.agents,
                        &self.models,
                        &mut self.shared,
                        proof,
                        &self.discovery,
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
                Section::Topology => orchestrator_studio::views::topology::show(
                    ui,
                    &mut self.dds,
                    &self.discovery,
                ),
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
    // Janela no tamanho do mockup (3.3 screen.png = 1600×1358) para que o
    // layout respire — janela pequena espreme os cards e o design some.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([1024.0, 640.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "DDS Orchestrator Studio",
        options,
        Box::new(|cc| {
            // Design system Stitch → egui (T-890-03 v6): superfícies, hairlines,
            // Inter + JetBrains Mono, accent ciano. Uma vez no boot.
            orchestrator_studio::theme::apply(&cc.egui_ctx);
            Ok(Box::new(StudioApp::new()))
        }),
    )
    .map_err(|err| anyhow::anyhow!("falha ao abrir a janela: {err}"))
}
