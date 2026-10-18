//! Painel 3.11 Topologia DDS & Enxame Distribuído (tela inicial do Studio).
//!
//! Refeito sobre o mockup Stitch atual + PRD v1.0: kicker com QoS do
//! contrato, janela em chips, faixa de 6 contadores, chips de filtro por
//! tópico canônico, mesh desenhado com `Painter` (cards clicáveis →
//! "ASSINANTE SELECIONADO") e painel de 7 abas — a primeira é TAREFAS DDS
//! (Tasks ao vivo) com barra de log de `TaskOutput` no rodapé.
//! Só dados reais: nada de GUID/RTT/throughput inventado.

use crate::dds_observe::{DdsSnapshot, DdsState, ToolRow};
use crate::discovery::DiscoveryState;
use crate::kit::{self, short_agent, short_host};
use crate::machines::{now_unix_ns, ProbeState};
use crate::theme;
use eframe::egui;

/// Presets de janela do design (dropdown 1/5/10/15/30 s).
const WINDOW_PRESETS: [u64; 5] = [1, 5, 10, 15, 30];

/// Abas do painel de coleções (índice = `DdsState::tab`; numeradas no design).
const TABS: [&str; 7] = [
    "1. Tarefas DDS",
    "2. Agentes DDS",
    "3. Server Status",
    "4. Tool Calls",
    "5. Métricas",
    "6. Descoberta DDS",
    "7. Instalações",
];

/// Tópicos canônicos filtráveis (PRD 3.11) → aba correspondente.
const TOPIC_TABS: [(&str, u8); 6] = [
    ("Tasks", 0),
    ("TaskOutput", 0),
    ("AgentRegistry", 1),
    ("ServerStatus", 2),
    ("ToolCall.Request", 3),
    ("Studio.NodePresence", 6),
];

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    // Drena o worker de observação (thread + mpsc — REQ/T-820-19).
    dds.poll();
    let drained = {
        let snapshot = &dds.snapshot;
        snapshot.tasks.len()
            + snapshot.task_outputs.len()
            + snapshot.agents.len()
            + snapshot.tools.len()
            + snapshot.metrics.len()
            + snapshot.discoveries.len()
            + snapshot.studio_nodes.len()
    };

    show_context(ui, dds, discovery, drained);
    show_counters(ui, dds, discovery, drained);
    show_tabs(ui, dds, discovery);
}

mod topology_context;
mod topology_counters;
mod topology_draw;
mod topology_draw_right;
mod topology_mesh;
mod topology_subscriber;
mod topology_tabs;
mod topology_tabs_a;
mod topology_tabs_b;
mod topology_tabs_c;

use topology_context::show_context;
use topology_counters::show_counters;
use topology_draw::draw_mesh;
use topology_mesh::{
    fit_text, join_preview, mesh_card, mesh_section_header, snapshot_has_data, tool_breakdown,
    topic_active_for_tab,
};
use topology_subscriber::selected_subscriber_card;
use topology_tabs::show_tabs;
use topology_tabs_a::{task_log_bar, tasks_tab};
use topology_tabs_b::{agents_tab, servers_tab, tools_tab};
use topology_tabs_c::{discovery_tab, installations_tab, metrics_tab};
