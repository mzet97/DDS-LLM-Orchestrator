//! Painel da origem remota: conexão ao nó + tabela de operações.

use crate::origin::optional_token;
use crate::state::AppState;
use eframe::egui;
use studio_node::protocol::AdminOp;

/// Resumo de uma linha para a tabela de operações do nó.
fn op_summary(op: &AdminOp) -> String {
    match op {
        AdminOp::Bootstrap { node_name } => format!("bootstrap {node_name}"),
        AdminOp::SetService { service, running } => {
            format!("serviço {service} {}", if *running { "on" } else { "off" })
        }
    }
}

/// Campo de URL, token (T-840-03a; fica só na memória da GUI), botão de
/// conexão e resumo/tabela do nó.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    node_url: &mut String,
    node_token: &mut String,
) {
    // Drena o worker de leitura do nó (thread + mpsc — REQ/T-820-19).
    state.poll();
    ui.horizontal(|ui| {
        ui.label("nó:");
        ui.text_edit_singleline(node_url);
        ui.label("token:");
        ui.add(egui::TextEdit::singleline(node_token).password(true));
        ui.add_enabled_ui(!state.busy(), |ui| {
            if ui.button("Conectar ao nó").clicked() {
                let url = node_url.clone();
                state.refresh_from_node_with_token(&url, optional_token(node_token));
            }
        });
    });
    if let Some(node) = state.node() {
        ui.separator();
        ui.label(format!(
            "nó: protocolo {}.{} · {} operação(ões)",
            node.version.major,
            node.version.minor,
            node.operations.len()
        ));
        if !node.operations.is_empty() {
            egui::Grid::new("node_ops_grid").show(ui, |ui| {
                ui.label("operation_id");
                ui.label("op");
                ui.end_row();
                for record in &node.operations {
                    ui.label(&record.id.0);
                    ui.label(op_summary(&record.op));
                    ui.end_row();
                }
            });
        }
    }
}
