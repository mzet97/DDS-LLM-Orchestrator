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

mod header_widget;
mod section;

use header_widget::{header_pill, HeaderPill};
use section::{Section, NAV_GROUPS};

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
    ssh: orchestrator_studio::ssh_session::SshSession,
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
            ssh: orchestrator_studio::ssh_session::SshSession::new(),
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

/// Parse de `STUDIO_SCROLL_Y` (capturas de verificação): `Some(y)` fixa o
/// scroll vertical; `None` (ausente/inválida/não-finita) = scroll livre.
fn parse_scroll_offset(raw: &str) -> Option<f32> {
    raw.trim().parse::<f32>().ok().filter(|y| y.is_finite())
}

mod app_impl;

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
