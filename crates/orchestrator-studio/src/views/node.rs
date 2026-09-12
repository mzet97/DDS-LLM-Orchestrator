//! Painel de nós: registro de nós conhecidos + conexão e operações.
//!
//! O registro guarda endereços digitados pelo operador (P3-admin parcial:
//! administrar N nós de uma GUI). Sem SSH aqui — G-02/G-04 pendentes.

use eframe::egui;
use orchestrator_studio::nodes::NodeRegistry;
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

/// Registro, conexão e resumo/tabela do nó selecionado.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, registry: &mut NodeRegistry) {
    ui.heading("Nós studio-node");
    ui.label("Só endereços que você digitou. Sem varredura, sem SSH (G-02/G-04 pendentes).");
    ui.horizontal(|ui| {
        ui.label("apelido:");
        ui.text_edit_singleline(&mut registry.draft_alias);
        ui.label("URL:");
        ui.text_edit_singleline(&mut registry.draft_url);
        if ui.button("Adicionar").clicked() {
            let alias = registry.draft_alias.clone();
            let url = registry.draft_url.clone();
            match registry.add(&alias, &url) {
                Ok(()) => {
                    registry.draft_alias.clear();
                    registry.draft_url.clear();
                    registry.error.clear();
                }
                Err(err) => {
                    registry.error = err.to_string();
                }
            }
        }
        if !registry.error.is_empty() {
            ui.label(&registry.error);
        }
    });
    if registry.list().is_empty() {
        ui.label("Nenhum nó cadastrado. Adicione o local (http://127.0.0.1:4317) ou um remoto.");
        return;
    }
    ui.horizontal(|ui| {
        let aliases: Vec<String> = registry
            .list()
            .iter()
            .map(|entry| entry.alias.clone())
            .collect();
        let selected_alias = registry.selected().map(|current| current.alias.clone());
        for alias in &aliases {
            if ui
                .selectable_label(selected_alias.as_deref() == Some(alias), alias)
                .clicked()
            {
                registry.select(alias);
            }
        }
        if ui.button("Remover selecionado").clicked() {
            if let Some(current) = registry.selected().map(|entry| entry.alias.clone()) {
                registry.remove(&current);
            }
        }
    });
    if let Some(current) = registry.selected() {
        ui.monospace(&current.url);
        if ui.button("Conectar ao nó").clicked() {
            state.refresh_from_node(&current.url.clone());
        }
    }
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
