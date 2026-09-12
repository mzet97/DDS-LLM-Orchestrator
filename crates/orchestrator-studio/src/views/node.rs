//! Painel da origem remota: conexão ao nó + tabela de operações.

use eframe::egui;
use orchestrator_studio::state::AppState;
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

/// Campo de URL, botão de conexão e resumo/tabela do nó.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, node_url: &mut String) {
    ui.horizontal(|ui| {
        ui.label("nó:");
        ui.text_edit_singleline(node_url);
        if ui.button("Conectar ao nó").clicked() {
            state.refresh_from_node(&node_url.clone());
        }
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
