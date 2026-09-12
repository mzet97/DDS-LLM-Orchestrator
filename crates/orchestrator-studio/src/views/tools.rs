//! Painel de ferramentas e gateways (T06): definição, restrição
//! declarada × observada e painel de teste. Tipo sem executor mostra
//! capacidade indisponível; nada executa shell/MCP improvisado.

use eframe::egui;
use orchestrator_studio::tools::{ToolImpl, ToolsPanel};

/// Lista, editor e teste da ferramenta selecionada.
pub fn show(ui: &mut egui::Ui, panel: &mut ToolsPanel) {
    ui.heading("Ferramentas e gateways");
    if ui.button("Nova ferramenta").clicked() {
        panel.create_draft();
    }
    if panel.tools.is_empty() {
        ui.label("Nenhuma ferramenta definida. Crie um rascunho para começar.");
        return;
    }
    let entries: Vec<(String, String, bool)> = panel
        .tools
        .iter()
        .map(|tool| {
            let name = if tool.name.is_empty() {
                tool.id.clone()
            } else {
                tool.name.clone()
            };
            let selected = panel.selected().map(|item| item.id.as_str()) == Some(tool.id.as_str());
            (tool.id.clone(), name, selected)
        })
        .collect();
    ui.horizontal(|ui| {
        for (id, name, selected) in &entries {
            if ui.selectable_label(*selected, name).clicked() {
                panel.select(id);
            }
        }
    });
    let Some(selected_id) = panel.selected().map(|item| item.id.clone()) else {
        ui.label("Selecione uma ferramenta para editar.");
        return;
    };
    let Some(tool) = panel.tools.iter_mut().find(|item| item.id == selected_id) else {
        return;
    };
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("nome:");
        ui.text_edit_singleline(&mut tool.name);
        ui.label("implementação:");
        egui::ComboBox::from_label("")
            .selected_text(tool.implementation.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut tool.implementation,
                    ToolImpl::Http,
                    ToolImpl::Http.label(),
                );
                ui.selectable_value(
                    &mut tool.implementation,
                    ToolImpl::Local,
                    ToolImpl::Local.label(),
                );
                ui.selectable_value(
                    &mut tool.implementation,
                    ToolImpl::Unsupported,
                    ToolImpl::Unsupported.label(),
                );
            });
    });
    ui.horizontal(|ui| {
        ui.label("gateway:");
        ui.text_edit_singleline(&mut tool.gateway);
        ui.label("máquina de execução:");
        ui.text_edit_singleline(&mut tool.machine);
    });
    ui.label("restrição declarada:");
    ui.text_edit_singleline(&mut tool.declared_restriction);
    ui.label(tool.restriction_line());
    if tool.implementation == ToolImpl::Unsupported {
        ui.group(|ui| {
            ui.strong("Requer implementação de executor");
            ui.label("Este tipo não executa nesta frente. Escrever arquivo ou chamar endpoint não é ação automática.");
        });
    }
}
