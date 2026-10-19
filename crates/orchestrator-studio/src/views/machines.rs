//! Painel 3.10 Máquinas / Descoberta (REQ/T-840-03 + T-890-03): fiel ao
//! mockup Stitch atual — hero com participantes/varredura/"Forçar probe
//! multicast", tabela canonical das instalações descobertas via
//! `Studio.NodePresence`, janela de lease, classificador de falha, mini
//! topologia do domínio, registro manual (Testar & Registrar) e catálogo
//! compartilhado persistido.
//!
//! Só dados reais: sem GUID/latência/ops inventados. O token digitado fica
//! SÓ na memória da GUI (RNF-04 / SDD §28): nunca é enviado ao catálogo,
//! persistido ou impresso. Probe 401 (nó conforme T-840-01) vira badge
//! roxo "falha de auth" — distinto de offline.

use crate::discovery::{DiscoveredNode, DiscoveryState};
use crate::kit;
use crate::machines::{now_unix_ns, MachinesState, ProbeState};
use crate::theme;
use eframe::egui;

/// Lease do tópico `Studio.NodePresence` (s) — barras da janela de lease.
pub fn show(ui: &mut egui::Ui, machines: &mut MachinesState, discovery: &mut DiscoveryState) {
    machines.poll();
    discovery.poll();
    // Probes manuais (Re-testar/modal/Testar) voltam em `machines.probes` —
    // espelha para as linhas da descoberta (a cada frame, após o `poll`,
    // para sobreviver à troca do snapshot do worker).
    for node in &mut discovery.nodes {
        let key = crate::machines::normalize_url(&node.url);
        if let Some(status) = machines.probes.get(&key) {
            node.probe = Some(status.clone());
        }
    }
    let domain = discovery.domain;
    let now_ns = now_unix_ns();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));

    show_hero(ui, machines, discovery, domain, now_ns);
    show_table(ui, machines, discovery, domain, now_ns);
    show_bottom(ui, machines, discovery, domain, now_ns);
}

mod machines_bottom;
mod machines_discovery;
mod machines_form;
mod machines_helpers;
mod machines_hero;
mod machines_mesh;
mod machines_table;

use machines_bottom::show_bottom;
use machines_helpers::{
    age_frac, auth_curto, card_frame, card_title, dot_color, heartbeat_label, hero_stat, port_of,
    truncate, HEARTBEAT_SECS, LEASE_SECS,
};
use machines_hero::show_hero;
use machines_table::show_table;

#[cfg(test)]
mod tests;
