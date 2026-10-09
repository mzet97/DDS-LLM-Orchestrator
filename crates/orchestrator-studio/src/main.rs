//! Casca eframe do Studio: navegação lateral + painel central (§30 SDD).
//!
//! Sem lógica de domínio aqui — cada painel lê capacidade real e o estado
//! testável vive nos módulos da lib.

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::catalog_remote::SharedCatalog;
use orchestrator_studio::inference::InferenceState;
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
/// Grupos da navegação (design Stitch: 5 grupos numerados + tag à direita).
const NAV_GROUPS: &[(&str, &str, &[Section])] = &[
    (
        "1. Sistema",
        "DDS",
        &[Section::Overview, Section::Topology, Section::Logs],
    ),
    (
        "2. Nós & Infraestrutura",
        "HOST",
        &[
            Section::Machines,
            Section::Node,
            Section::Services,
            Section::Shared,
        ],
    ),
    (
        "3. Enxame IA & Inferência",
        "AI",
        &[
            Section::Agents,
            Section::Inference,
            Section::Launch,
            Section::Workflow,
            #[cfg(feature = "dds")]
            Section::Tools,
        ],
    ),
    ("4. Artefatos", "BIN", &[Section::Models]),
    ("5. Diagnóstico", "OPS", &[Section::Dispatch]),
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
            Self::Overview => "Visão Geral",
            Self::Node => "Nó studio-node",
            Self::Inference => "Inferência",
            Self::Launch => "Subir Inferência",
            Self::Agents => "Agentes",
            Self::Dispatch => "Despacho",
            Self::Models => "Modelos GGUF",
            Self::Services => "Serviços",
            Self::Shared => "Catálogo Compartilhado",
            Self::Machines => "Máquinas",
            Self::Topology => "Topologia DDS",
            Self::Workflow => "Workflow",
            #[cfg(feature = "dds")]
            Self::Tools => "Ferramentas & Tool Calls",
            Self::Logs => "Logs da GUI",
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

    /// Tela pela numeração do mockup ("3.1".."3.14") — `STUDIO_SCREEN`
    /// (capturas de verificação/testes manuais). Fora da lista → `None`.
    fn from_num(num: &str) -> Option<Self> {
        Some(match num.trim() {
            "3.1" => Self::Overview,
            "3.2" => Self::Node,
            "3.3" => Self::Inference,
            "3.4" => Self::Launch,
            "3.5" => Self::Agents,
            "3.6" => Self::Dispatch,
            "3.7" => Self::Models,
            "3.8" => Self::Services,
            "3.9" => Self::Shared,
            "3.10" => Self::Machines,
            "3.11" => Self::Topology,
            "3.12" => Self::Workflow,
            #[cfg(feature = "dds")]
            "3.13" => Self::Tools,
            "3.14" => Self::Logs,
            _ => return None,
        })
    }

    /// Tag curta mono da sidebar (design Stitch: DOM/MESH/500/AUTO/…).
    fn tag(self) -> &'static str {
        match self {
            Self::Overview => "DOM",
            Self::Node => "PROT",
            Self::Inference => "LLAMA",
            Self::Launch => "RUN",
            Self::Agents => "HB",
            Self::Dispatch => ":8080",
            Self::Models => "SHA256",
            Self::Services => "SYSTEMD",
            Self::Shared => "REV409",
            Self::Machines => "AUTO",
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
    runner: orchestrator_studio::runner::RunnerState,
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
    /// Estado de apresentação do painel de Logs (tela 3.14).
    logs_panel: orchestrator_studio::studio_log::LogsPanel,
    go_chat: bool,
    /// Sidebar recolhida (ícone de layout no header) — só numerações.
    sidebar_collapsed: bool,
}

/// Hash curto do git da árvore (build.rs; "dev" fora de repositório).
const GIT_HASH: &str = match option_env!("STUDIO_GIT_HASH") {
    Some(hash) => hash,
    None => "dev",
};

/// Especificação de um pill do header (largura medida do conteúdo — nunca
/// sobrepõe textos vizinhos, como o NÓ ALVO fazia com largura fixa).
struct HeaderPill<'a> {
    text: &'a str,
    text_color: egui::Color32,
    /// Segunda cor no mesmo pill (ex.: estado de conexão após a URL).
    suffix: Option<(&'a str, egui::Color32)>,
    fill: egui::Color32,
    stroke: egui::Color32,
    dot_left: Option<egui::Color32>,
    dot_right: Option<egui::Color32>,
    clickable: bool,
}

/// Desenha um pill do header e devolve a resposta (clique quando clicável).
fn header_pill(ui: &mut egui::Ui, spec: &HeaderPill) -> egui::Response {
    let font = egui::FontId::monospace(10.5);
    let main = ui
        .painter()
        .layout_no_wrap(spec.text.to_owned(), font.clone(), spec.text_color);
    let sub = spec
        .suffix
        .map(|(text, color)| ui.painter().layout_no_wrap(text.to_owned(), font, color));
    let mut width = 20.0 + main.size().x;
    if spec.dot_left.is_some() {
        width += 12.0;
    }
    if let Some(galley) = &sub {
        width += 8.0 + galley.size().x;
    }
    if spec.dot_right.is_some() {
        width += 12.0;
    }
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 26.0),
        if spec.clickable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let radius = egui::CornerRadius::same(orchestrator_studio::theme::RADIUS_MD as u8);
    ui.painter().rect_filled(rect, radius, spec.fill);
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, spec.stroke),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 10.0;
    if let Some(color) = spec.dot_left {
        ui.painter()
            .circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.0, color);
        x += 12.0;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - main.size().y / 2.0),
        main.clone(),
        spec.text_color,
    );
    x += main.size().x;
    if let Some(galley) = &sub {
        x += 8.0;
        ui.painter().galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley.clone(),
            spec.suffix
                .map(|(_, color)| color)
                .unwrap_or(spec.text_color),
        );
        x += galley.size().x;
    }
    if let Some(color) = spec.dot_right {
        ui.painter()
            .circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.0, color);
    }
    if spec.clickable && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

impl StudioApp {
    fn new() -> Self {
        Self {
            // Tela inicial = Topologia DDS (decisão do autor 2026-10-06): o
            // mapa vivo do domínio é a porta de entrada; sem a feature `dds`
            // o fallback honesto é a Visão geral. `STUDIO_SCREEN` sobrepõe
            // (capturas de verificação) sem mudar o padrão.
            section: std::env::var("STUDIO_SCREEN")
                .ok()
                .and_then(|num| Section::from_num(&num))
                .unwrap_or({
                    #[cfg(feature = "dds")]
                    {
                        Section::Topology
                    }
                    #[cfg(not(feature = "dds"))]
                    {
                        Section::Overview
                    }
                }),
            state: AppState::new(),
            node_url: String::from("http://127.0.0.1:4317"),
            node_token: String::new(),
            inference: InferenceState::new(),
            runner: orchestrator_studio::runner::RunnerState::new(),
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
            logs_panel: orchestrator_studio::studio_log::LogsPanel::new(),
            go_chat: false,
            sidebar_collapsed: false,
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
            .exact_size(44.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // Logo do produto (design: flor ciano de 6 pétalas).
                    let (logo_rect, _) =
                        ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    let center = logo_rect.center();
                    for petal in 0..6 {
                        let angle = petal as f32 * std::f32::consts::PI / 3.0;
                        ui.painter().circle_filled(
                            egui::pos2(center.x + angle.cos() * 6.0, center.y + angle.sin() * 6.0),
                            2.6,
                            orchestrator_studio::theme::PRIMARY_CONTAINER,
                        );
                    }
                    ui.painter().circle_filled(
                        center,
                        2.2,
                        orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                    );
                    ui.label(
                        egui::RichText::new("DDS Orchestrator Studio")
                            .size(15.0)
                            .strong()
                            .color(orchestrator_studio::theme::ON_SURFACE),
                    );
                    ui.label(
                        egui::RichText::new(format!("v1.0-{GIT_HASH}"))
                            .monospace()
                            .size(10.0)
                            .color(orchestrator_studio::theme::OUTLINE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Avatar do operador (design: círculo claro à direita).
                        let (avatar_rect, _) =
                            ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                        let avatar = avatar_rect.center();
                        ui.painter().circle_filled(
                            avatar,
                            13.0,
                            orchestrator_studio::theme::SECONDARY_FIXED,
                        );
                        ui.painter().circle_filled(
                            egui::pos2(avatar.x, avatar.y - 4.0),
                            4.0,
                            orchestrator_studio::theme::SURFACE,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_center_size(
                                egui::pos2(avatar.x, avatar.y + 7.5),
                                egui::vec2(13.0, 5.0),
                            ),
                            2.0,
                            orchestrator_studio::theme::SURFACE,
                        );
                        // Alternador de layout (design: ícone de painel) —
                        // recolhe a sidebar para só numerações.
                        let (layout_rect, layout_toggle) =
                            ui.allocate_exact_size(egui::vec2(30.0, 26.0), egui::Sense::click());
                        ui.painter().rect_filled(
                            layout_rect,
                            egui::CornerRadius::same(6),
                            orchestrator_studio::theme::SURFACE_HIGH,
                        );
                        ui.painter().rect_stroke(
                            layout_rect,
                            egui::CornerRadius::same(6),
                            egui::Stroke::new(1.0, orchestrator_studio::theme::SURFACE_HIGHEST),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(layout_rect.left() + 6.0, layout_rect.top() + 6.0),
                                egui::vec2(6.0, 14.0),
                            ),
                            1.5,
                            orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(layout_rect.left() + 15.0, layout_rect.top() + 6.0),
                                egui::vec2(9.0, 4.0),
                            ),
                            1.0,
                            orchestrator_studio::theme::OUTLINE,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(layout_rect.left() + 15.0, layout_rect.top() + 12.0),
                                egui::vec2(9.0, 8.0),
                            ),
                            1.0,
                            orchestrator_studio::theme::SURFACE_HIGHEST,
                        );
                        if layout_toggle.clicked() {
                            self.sidebar_collapsed = !self.sidebar_collapsed;
                        }
                        if layout_toggle.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        // Modo protegido (design: pill com escudo + ação).
                        let armed = self.protected.armed;
                        let prot = header_pill(
                            ui,
                            &HeaderPill {
                                text: if armed {
                                    "MODO PROTEGIDO: ARMADO (Clique para Desarmar)"
                                } else {
                                    "MODO PROTEGIDO: DESARMADO (Clique para Armar)"
                                },
                                text_color: if armed {
                                    orchestrator_studio::theme::ERROR
                                } else {
                                    orchestrator_studio::theme::ON_SURFACE_VARIANT
                                },
                                suffix: None,
                                fill: if armed {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::ERROR,
                                        14,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGH
                                },
                                stroke: if armed {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::ERROR,
                                        60,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGHEST
                                },
                                dot_left: None,
                                dot_right: None,
                                clickable: true,
                            },
                        );
                        if prot.clicked() {
                            self.protected.set_armed(!armed);
                        }
                        if let Some(refusal) = &self.protected.last_refusal {
                            if armed {
                                ui.label(
                                    egui::RichText::new(refusal.as_str())
                                        .small()
                                        .color(orchestrator_studio::theme::ERROR),
                                );
                            }
                        }
                        // Auto-Carga (design: pill com dot ciano à direita).
                        let auto = header_pill(
                            ui,
                            &HeaderPill {
                                text: "Auto-Carga (5s)",
                                text_color: if self.auto_reload {
                                    orchestrator_studio::theme::PRIMARY_FIXED_DIM
                                } else {
                                    orchestrator_studio::theme::STALE
                                },
                                suffix: None,
                                fill: if self.auto_reload {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                                        12,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGH
                                },
                                stroke: if self.auto_reload {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                                        55,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGHEST
                                },
                                dot_left: None,
                                dot_right: Some(if self.auto_reload {
                                    orchestrator_studio::theme::PRIMARY_CONTAINER
                                } else {
                                    orchestrator_studio::theme::STALE
                                }),
                                clickable: true,
                            },
                        );
                        if auto.clicked() {
                            self.auto_reload = !self.auto_reload;
                            orchestrator_studio::studio_log::info(format!(
                                "auto-carga contínua: {}",
                                if self.auto_reload {
                                    "ligada"
                                } else {
                                    "desligada"
                                }
                            ));
                        }
                        // NÓ ALVO (design: URL + estado + ▾ num pill só).
                        if let Some(target) = self.discovery.selected_url() {
                            let connected = self.state.node().is_some();
                            let state_text = if connected {
                                format!(
                                    "● Conectado v{}.{} ▾",
                                    self.state.node().map(|n| n.version.major).unwrap_or(1),
                                    self.state.node().map(|n| n.version.minor).unwrap_or(0)
                                )
                            } else {
                                String::from("○ conectando…")
                            };
                            header_pill(
                                ui,
                                &HeaderPill {
                                    text: &format!("NÓ ALVO: {target}"),
                                    text_color: orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                                    suffix: Some((
                                        &state_text,
                                        if connected {
                                            orchestrator_studio::theme::OK
                                        } else {
                                            orchestrator_studio::theme::WARN
                                        },
                                    )),
                                    fill: orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                                        12,
                                    ),
                                    stroke: orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                                        55,
                                    ),
                                    dot_left: None,
                                    dot_right: None,
                                    clickable: false,
                                },
                            );
                        }
                        // Domínio (design: pill ciano com dot pulsante).
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
                        header_pill(
                            ui,
                            &HeaderPill {
                                text: &format!("DDS DOMAIN {}", self.discovery.domain),
                                text_color: orchestrator_studio::theme::ON_SURFACE_VARIANT,
                                suffix: None,
                                fill: orchestrator_studio::theme::tint(
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                    10,
                                ),
                                stroke: orchestrator_studio::theme::tint(
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                    45,
                                ),
                                dot_left: Some(
                                    orchestrator_studio::theme::PRIMARY_CONTAINER
                                        .gamma_multiply(1.0 - pulse * 0.5),
                                ),
                                dot_right: None,
                                clickable: false,
                            },
                        );
                    });
                });
            });

        // T-890-03: descoberta → alvo único. O nó selecionado (auto: primeiro
        // online; manual: combobox no painel Máquinas) propaga para TODOS os
        // painéis que falam com um studio-node — trocar uma vez, muda tudo.
        self.discovery.poll();
        // `STUDIO_TARGET_URL` (capturas de verificação): força o alvo para a
        // URL dada assim que ela aparece na descoberta — determinístico. Sem
        // a env, valem auto-seleção/seleção manual normais.
        if let Ok(wanted) = std::env::var("STUDIO_TARGET_URL") {
            let wanted = wanted.trim().to_string();
            if !wanted.is_empty()
                && self.discovery.selected_url().as_deref() != Some(wanted.as_str())
            {
                if let Some(index) = self
                    .discovery
                    .nodes
                    .iter()
                    .position(|node| node.url == wanted)
                {
                    self.discovery.select(index);
                    orchestrator_studio::studio_log::info(format!("alvo forçado (env): {wanted}"));
                }
            }
        }
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
                self.machines.refresh();
            }
        }

        // Auto-carga contínua (toggle do header, mockup "Auto-Carga (5s)"):
        // re-leituras periódicas do alvo — cada refresh é no-op enquanto busy.
        if self.auto_reload {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
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
                    self.machines.refresh();
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
            .exact_size(56.0)
            .resizable(false)
            .show(ui, |ui| {
                let right_w = 250.0;
                let left_w = (ui.available_width() - right_w - 12.0).max(200.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(left_w);
                        ui.set_max_width(left_w);
                        // Linha 1: estado do nó alvo (design: ● + versão + ops).
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("ESTADO DO NÓ:")
                                    .monospace()
                                    .size(10.5)
                                    .color(orchestrator_studio::theme::OUTLINE),
                            );
                            match self.state.node() {
                                Some(node) => {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "● Conectado (studio-node v{}.{} · {} ops registradas)",
                                            node.version.major,
                                            node.version.minor,
                                            node.operations.len()
                                        ))
                                        .monospace()
                                        .size(10.5)
                                        .color(orchestrator_studio::theme::PRIMARY_FIXED_DIM),
                                    );
                                }
                                None => {
                                    ui.label(
                                        egui::RichText::new(format!("○ {}", self.state.status()))
                                            .monospace()
                                            .size(10.5)
                                            .color(
                                                orchestrator_studio::theme::ON_SURFACE_VARIANT,
                                            ),
                                    );
                                }
                            }
                        });
                        // Linha 2: descoberta DDS viva (design: contagens).
                        let servers = if self.discovery.servers.is_empty() {
                            String::from("0 servidor inferência")
                        } else if self.discovery.servers.len() == 1 {
                            format!(
                                "1 servidor inferência ({})",
                                self.discovery.servers[0].server_id
                            )
                        } else {
                            format!(
                                "{} servidores inferência",
                                self.discovery.servers.len()
                            )
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "DESCOBERTA DDS: {} nós ativos (lease 10s) · {} agentes registrados · {servers}",
                                self.discovery.nodes.len(),
                                self.discovery.agents.len(),
                            ))
                            .monospace()
                            .size(10.5)
                            .color(orchestrator_studio::theme::ON_SURFACE_VARIANT),
                        );
                        // Linha 3: alvo + estado do token (só memória, nunca disco).
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "ALVO SELECIONADO: {}",
                                    self.discovery.selected_url().unwrap_or_else(|| {
                                        String::from("— nenhum alvo —")
                                    })
                                ))
                                .monospace()
                                .size(10.5)
                                .color(orchestrator_studio::theme::ON_SURFACE_VARIANT),
                            );
                            ui.label(
                                egui::RichText::new(if self.node_token.is_empty() {
                                    "[sem token em memória]"
                                } else {
                                    "[Bearer Token OK em memória]"
                                })
                                .monospace()
                                .size(10.5)
                                .color(if self.node_token.is_empty() {
                                    orchestrator_studio::theme::OUTLINE
                                } else {
                                    orchestrator_studio::theme::PRIMARY_FIXED_DIM
                                }),
                            );
                        });
                    });
                    // Bloco direito (design: latência + domínio + sem segredos;
                    // rect fixo + painter: sem ninho de layouts).
                    let ema_text = self
                        .state
                        .rtt_ema_ms()
                        .map(|ema| format!("{ema:.1}ms"))
                        .unwrap_or_else(|| String::from("—"));
                    let domain_text = self.discovery.domain.to_string();
                    let right_rows: [(&str, &str, egui::Color32); 3] = [
                        (
                            "Latência HTTP:",
                            ema_text.as_str(),
                            orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                        ),
                        (
                            "Domínio:",
                            domain_text.as_str(),
                            orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                        ),
                        (
                            "",
                            "Sem segredos em disco",
                            orchestrator_studio::theme::OUTLINE,
                        ),
                    ];
                    let (right_rect, _) =
                        ui.allocate_exact_size(egui::vec2(right_w, 54.0), egui::Sense::hover());
                    let right_font = egui::FontId::monospace(10.5);
                    for (index, (label, value, color)) in right_rows.iter().enumerate() {
                        let y = right_rect.top() + index as f32 * 18.0;
                        let value_galley = ui.painter().layout_no_wrap(
                            (*value).to_owned(),
                            right_font.clone(),
                            *color,
                        );
                        let mut x = right_rect.right() - value_galley.size().x;
                        ui.painter().galley(egui::pos2(x, y), value_galley, *color);
                        if !label.is_empty() {
                            let label_galley = ui.painter().layout_no_wrap(
                                (*label).to_owned(),
                                right_font.clone(),
                                orchestrator_studio::theme::OUTLINE,
                            );
                            x -= 6.0 + label_galley.size().x;
                            ui.painter().galley(
                                egui::pos2(x, y),
                                label_galley,
                                orchestrator_studio::theme::OUTLINE,
                            );
                        }
                    }
                });
            });
        egui::Panel::left("nav")
            .exact_size(if self.sidebar_collapsed { 52.0 } else { 248.0 })
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(orchestrator_studio::theme::SPACE_SM);
                for (group, group_tag, sections) in NAV_GROUPS {
                    if self.sidebar_collapsed {
                        // Recolhida: só numerações clicáveis.
                        for section in *sections {
                            let selected = self.section == *section;
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 26.0),
                                egui::Sense::click(),
                            );
                            if selected {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(4),
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                );
                            }
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                section.num(),
                                egui::FontId::monospace(9.5),
                                if selected {
                                    orchestrator_studio::theme::ON_PRIMARY
                                } else {
                                    orchestrator_studio::theme::OUTLINE
                                },
                            );
                            if response.clicked() {
                                self.section = *section;
                            }
                        }
                        ui.add_space(orchestrator_studio::theme::SPACE_SM);
                        continue;
                    }
                    // Cabeçalho do grupo: rótulo + tag pill à direita.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new((*group).to_uppercase())
                                .monospace()
                                .small()
                                .color(orchestrator_studio::theme::OUTLINE),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tag = ui.painter().layout_no_wrap(
                                (*group_tag).to_owned(),
                                egui::FontId::monospace(9.0),
                                orchestrator_studio::theme::OUTLINE,
                            );
                            let (tag_rect, _) = ui.allocate_exact_size(
                                egui::vec2(tag.size().x + 12.0, 15.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                tag_rect,
                                egui::CornerRadius::same(4),
                                orchestrator_studio::theme::SURFACE_HIGH,
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    tag_rect.center().x - tag.size().x / 2.0,
                                    tag_rect.center().y - tag.size().y / 2.0,
                                ),
                                tag,
                                orchestrator_studio::theme::OUTLINE,
                            );
                        });
                    });
                    ui.add_space(orchestrator_studio::theme::SPACE_XS);
                    for section in *sections {
                        let busy = matches!(section, Section::Models) && self.models.is_busy();
                        let base = section.label();
                        let label = if busy {
                            format!("{base} …")
                        } else {
                            base.to_owned()
                        };
                        let selected = self.section == *section;
                        // Linha desenhada à mão (design: selecionado = ciano
                        // sólido com texto escuro + tag pill à direita).
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 30.0),
                            egui::Sense::click(),
                        );
                        if selected {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(6),
                                orchestrator_studio::theme::PRIMARY_CONTAINER,
                            );
                        } else if response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(6),
                                orchestrator_studio::theme::tint(
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                    8,
                                ),
                            );
                        }
                        let num_font = egui::FontId::monospace(10.5);
                        let num_size = ui
                            .painter()
                            .layout_no_wrap(
                                section.num().to_owned(),
                                num_font.clone(),
                                orchestrator_studio::theme::OUTLINE,
                            )
                            .size()
                            .x;
                        ui.painter().text(
                            egui::pos2(rect.left() + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            section.num(),
                            num_font,
                            if selected {
                                orchestrator_studio::theme::ON_PRIMARY
                            } else {
                                orchestrator_studio::theme::OUTLINE
                            },
                        );
                        // Tag pill à direita (mede antes para truncar o rótulo).
                        let tag_font = egui::FontId::monospace(8.5);
                        let tag_text = ui.painter().layout_no_wrap(
                            section.tag().to_owned(),
                            tag_font.clone(),
                            orchestrator_studio::theme::OUTLINE,
                        );
                        let pill_w = tag_text.size().x + 12.0;
                        let pill_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.right() - 8.0 - pill_w, rect.center().y - 8.0),
                            egui::vec2(pill_w, 16.0),
                        );
                        ui.painter().rect_filled(
                            pill_rect,
                            egui::CornerRadius::same(8),
                            if selected {
                                // Ciano escuro sólido: legível sobre o ciano cheio.
                                egui::Color32::from_rgb(0x00, 0x7a, 0x8a)
                            } else {
                                orchestrator_studio::theme::SURFACE_HIGH
                            },
                        );
                        ui.painter().galley(
                            egui::pos2(
                                pill_rect.center().x - tag_text.size().x / 2.0,
                                pill_rect.center().y - tag_text.size().y / 2.0,
                            ),
                            tag_text,
                            if selected {
                                orchestrator_studio::theme::ON_PRIMARY
                            } else {
                                orchestrator_studio::theme::OUTLINE
                            },
                        );
                        // Rótulo truncado no espaço restante.
                        let label_x = rect.left() + 10.0 + num_size + 8.0;
                        let label_max = pill_rect.left() - 6.0 - label_x;
                        let label_font = egui::FontId::proportional(11.5);
                        let mut shown = label.clone();
                        while ui
                            .painter()
                            .layout_no_wrap(
                                shown.clone(),
                                label_font.clone(),
                                orchestrator_studio::theme::ON_SURFACE,
                            )
                            .size()
                            .x
                            > label_max.max(20.0)
                            && shown.len() > 4
                        {
                            shown.pop();
                        }
                        if shown.len() != label.len() {
                            shown.push('…');
                        }
                        ui.painter().text(
                            egui::pos2(label_x, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            shown,
                            label_font,
                            if selected {
                                orchestrator_studio::theme::ON_PRIMARY
                            } else {
                                orchestrator_studio::theme::ON_SURFACE
                            },
                        );
                        if response.clicked() {
                            self.section = *section;
                        }
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                    ui.add_space(orchestrator_studio::theme::SPACE_MD);
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            // `id_salt` por tela: cada aba guarda sua posição; sem isso as
            // 14 telas compartilhariam um único offset persistente.
            let scroll_area = egui::ScrollArea::vertical().id_salt(self.section.num());
            // `STUDIO_SCROLL_Y` fixa o scroll (capturas de verificação).
            // Ausente/inválida → scroll livre: `.vertical_scroll_offset()`
            // é reaplicado TODO frame pelo egui e travaria a roda em 0.
            let fixed_y: Option<f32> = std::env::var("STUDIO_SCROLL_Y")
                .ok()
                .and_then(|raw| parse_scroll_offset(&raw));
            let scroll_area = match fixed_y {
                Some(y) => scroll_area.vertical_scroll_offset(y),
                None => scroll_area,
            };
            scroll_area.show(ui, |ui| match self.section {
                Section::Overview => {
                    let proved = self.runner.steps.len() == 5
                        && self.runner.steps.iter().all(|step| step.ok);
                    let proof = if proved {
                        self.runner
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
                        &self.discovery,
                        &self.protected,
                    );
                }
                Section::Inference => orchestrator_studio::views::inference::show(
                    ui,
                    &mut self.inference,
                    &self.discovery,
                ),
                Section::Launch => {
                    orchestrator_studio::views::launch::show(
                        ui,
                        &mut self.runner,
                        &mut self.go_chat,
                        &self.discovery,
                    );
                }
                Section::Agents => {
                    orchestrator_studio::views::agents::show(ui, &mut self.agents, &self.discovery)
                }
                Section::Models => orchestrator_studio::views::models::show(ui, &mut self.models),
                Section::Services => orchestrator_studio::views::services::show(
                    ui,
                    &mut self.services,
                    &mut self.protected,
                    &self.discovery,
                ),
                Section::Shared => {
                    orchestrator_studio::views::shared_catalog::show(ui, &mut self.shared);
                }
                Section::Machines => orchestrator_studio::views::machines::show(
                    ui,
                    &mut self.machines,
                    &mut self.discovery,
                ),
                Section::Dispatch => orchestrator_studio::views::dispatch::show(
                    ui,
                    &mut self.dispatch,
                    &self.discovery,
                ),
                #[cfg(feature = "dds")]
                Section::Topology => {
                    orchestrator_studio::views::topology::show(ui, &mut self.dds, &self.discovery)
                }
                Section::Workflow => {
                    orchestrator_studio::views::workflow::show(ui, &mut self.workflow)
                }
                #[cfg(feature = "dds")]
                Section::Tools => {
                    orchestrator_studio::views::tools::show(ui, &mut self.dds, &self.protected)
                }
                Section::Logs => orchestrator_studio::views::logs::show(ui, &mut self.logs_panel),
                #[cfg(not(feature = "dds"))]
                Section::Topology => {
                    ui.heading("Topologia DDS");
                    ui.label(
                        "Recompile com --features dds para observar Tasks/TaskOutput ao vivo.",
                    );
                }
            });
            if self.go_chat {
                self.go_chat = false;
                self.section = Section::Inference;
            }
        });
    }
}

/// Parse de `STUDIO_SCROLL_Y` (capturas de verificação): `Some(y)` fixa o
/// scroll vertical; `None` (ausente/inválida/não-finita) = scroll livre.
fn parse_scroll_offset(raw: &str) -> Option<f32> {
    raw.trim().parse::<f32>().ok().filter(|y| y.is_finite())
}

fn main() -> Result<()> {
    // Janela no tamanho dos mockups Stitch (UX3: 1500×950, min 1280×800 —
    // janela pequena espreme os cards e o design some). `STUDIO_WIN_H`
    // sobrepõe a altura (capturas de verificação do conteúdo longo).
    let win_h: f32 = std::env::var("STUDIO_WIN_H")
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .filter(|h: &f32| *h >= 800.0)
        .unwrap_or(950.0);
    // `STUDIO_ONTOP=1` (capturas de verificação): `AlwaysOnTop` — no
    // Wayland o compositor só manda frame callbacks para janela visível, e
    // sem callbacks o eframe não entrega `RedrawRequested` (janela ocluída
    // congela no 1º frame). Sem a env, nível normal.
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1500.0, win_h])
        .with_min_inner_size([1280.0, 800.0]);
    let viewport = if std::env::var("STUDIO_ONTOP").is_ok_and(|v| v == "1") {
        viewport.with_window_level(egui::WindowLevel::AlwaysOnTop)
    } else {
        viewport
    };
    let options = eframe::NativeOptions {
        viewport,
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "DDS Orchestrator Studio",
        options,
        Box::new(|cc| {
            // Design system Stitch → egui (T-890-03 v6): superfícies, hairlines,
            // Inter + JetBrains Mono, accent ciano. Uma vez no boot.
            orchestrator_studio::theme::apply(&cc.egui_ctx);
            // Watchdog de repaint: o agendamento de `request_repaint_after`
            // perde uma corrida no arranque frio (evento descartado como
            // "outdated" + supressão de duplicata no ctx = loop parado, UI
            // congelada no 1º frame — verificado por captura 2026-10-08).
            // Um `request_repaint` externo periódico (2 Hz, custo
            // desprezível) garante o destravamento e countdowns vivos.
            let repaint_ctx = cc.egui_ctx.clone();
            if std::thread::Builder::new()
                .name(String::from("studio-repaint-watchdog"))
                .spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    repaint_ctx.request_repaint();
                })
                .is_err()
            {
                eprintln!("studio: aviso: watchdog de repaint não subiu");
            }
            Ok(Box::new(StudioApp::new()))
        }),
    )
    .map_err(|err| anyhow::anyhow!("falha ao abrir a janela: {err}"))
}

#[cfg(test)]
mod scroll_tests {
    use super::parse_scroll_offset;

    #[test]
    fn accepts_plain_number_when_env_set() {
        // Given: valor de STUDIO_SCROLL_Y bem-formado / When: parse / Then: offset fixo.
        assert_eq!(parse_scroll_offset("120"), Some(120.0));
    }

    #[test]
    fn trims_whitespace_when_env_has_padding() {
        // Given: "  45.5 " / When: parse / Then: 45.5.
        assert_eq!(parse_scroll_offset("  45.5 "), Some(45.5));
    }

    #[test]
    fn rejects_garbage_when_env_invalid() {
        // Given: texto não-numérico / When: parse / Then: None = scroll livre.
        assert_eq!(parse_scroll_offset("abc"), None);
    }

    #[test]
    fn rejects_non_finite_when_env_degenerate() {
        // Given: NaN/inf / When: parse / Then: None (nunca trava o egui com NaN).
        assert_eq!(parse_scroll_offset("NaN"), None);
        assert_eq!(parse_scroll_offset("inf"), None);
    }
}
