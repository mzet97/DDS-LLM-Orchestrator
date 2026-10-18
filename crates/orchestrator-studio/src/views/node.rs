//! Painel 3.2 Nó studio-node: fiel ao mockup Stitch — cabeçalho SEC:3.2,
//! card de conexão RPC + token em memória, status (protocolo · RTT medido ·
//! socket · modo), 3 cards (protocolo · operações · concessão/lease), log de
//! operações com filtro + inspetor JSON e card de falhas 401×rede.
//!
//! Só dados reais: `OpRecord = id + op` (sem timestamp/retorno por operação
//! no contrato — colunas inexistentes ficam de fora, com nota); versão é
//! `ProtocolVersion { major, minor }` do `GET /version`; sem GUID/fd/slots
//! inventados. GUID do design (DDS_GUID) não existe nos dados e foi omitido.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::now_unix_ns;
use crate::origin::optional_token;
use crate::protected::ProtectedGuard;
use crate::state::AppState;
use crate::theme;
use eframe::egui;
use studio_node::protocol::AdminOp;

/// Lease do tópico `Studio.NodePresence` (s) — card de concessão.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    node_url: &mut String,
    node_token: &mut String,
    discovery: &DiscoveryState,
    _guard: &ProtectedGuard,
) {
    // Drena o worker de leitura do nó (thread + mpsc — REQ/T-820-19).
    state.poll();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));

    show_header(ui, state, node_url, node_token, discovery);
    show_connection(ui, state, node_url, node_token, discovery);
    show_cards(ui, state, node_url, node_token, discovery);
    show_ops_section(ui, state, node_url, node_token, discovery);
    show_failures(ui, state, node_url, node_token, discovery);
}

mod node_cards;
mod node_connection;
mod node_failures;
mod node_header;
mod node_helpers;
mod node_ops;
mod node_ops_section;

use node_cards::show_cards;
use node_connection::show_connection;
use node_failures::show_failures;
use node_header::show_header;
use node_helpers::{
    card_frame, card_title, live_state, op_kind, op_summary, op_target, LEASE_SECS,
};
use node_ops::{show_inspector, show_ops_log};
use node_ops_section::show_ops_section;
