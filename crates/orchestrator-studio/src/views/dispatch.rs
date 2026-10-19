//! Painel 3.6 Despacho (DIAG-3.6): teste síncrono direto ao orquestrador
//! (`POST /api/v1/chat/completions/sync`) — parâmetros reais à esquerda,
//! card de resultado com decomposição T1–T6 à direita e histórico da sessão.
//!
//! Só dados reais: temperatura/max_tokens/timeout vão de verdade no fio;
//! stop-sequence, tok/s, estratégia e fila global do mockup NÃO existem no
//! contrato (o backend zera `tokens_*` e não tem `stop`) e ficam FORA.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use crate::workload::DispatchState;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, dispatch: &mut DispatchState, discovery: &DiscoveryState) {
    panel_header(
        ui,
        "DIAG-3.6",
        "3.6 Despacho de Tarefas",
        "Teste Síncrono Direto contra Orquestrador HTTP · Bypass de Workflows para Calibração de Nó",
    );
    ui.label(
        egui::RichText::new("● POST /api/v1/chat/completions/sync")
            .monospace()
            .small()
            .color(theme::PRIMARY_FIXED_DIM),
    );
    ui.add_space(theme::SPACE_XS);

    // Drena o worker de despacho (thread + mpsc — REQ/T-820-19).
    dispatch.poll();

    // ── Chips de estado + RESET à direita ──
    ui.horizontal(|ui| {
        let (conn_text, conn_color) = conn_chip(dispatch);
        kit::badge(ui, &conn_text, conn_color);
        let pool = discovery.agents.len();
        kit::badge(
            ui,
            &if pool == 1 {
                String::from("1 Agente no Pool")
            } else {
                format!("{pool} Agentes no Pool")
            },
            if pool == 0 {
                theme::STALE
            } else {
                theme::PRIMARY_FIXED_DIM
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("↻ RESET DE DIAGNÓSTICO").clicked() {
                dispatch.reset();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    ui.columns(2, |columns| {
        params_column(&mut columns[0], dispatch, discovery);
        result_column(&mut columns[1], dispatch);
    });

    ui.add_space(theme::SPACE_MD);
    history_section(ui, dispatch);
}

/// Chip de conexão: último estado CONHECIDO do orquestrador (nunca sonda
/// sozinho — só reflete despachos que o operador pediu).
mod dispatch_history;
mod dispatch_params;
mod dispatch_result;

use dispatch_history::history_section;
use dispatch_params::{caption, conn_chip, header, params_column};
use dispatch_result::result_column;

#[cfg(test)]
mod tests;
