//! Painel de orquestradores (T09): definição do monitor, destino,
//! domínio, configuração suportada, instâncias e evidências.

use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::orchestrators::{MonitorDefinition, OrchestratorPanel};

/// Definição em edição + contagens observadas (sem posar de executor).
pub fn show(ui: &mut egui::Ui, panel: &mut OrchestratorPanel, agents: &AgentsState) {
    ui.heading("Orquestradores");
    ui.label(MonitorDefinition::role());
    ui.horizontal(|ui| {
        ui.label("apelido do monitor:");
        ui.text_edit_singleline(&mut panel.draft_alias);
        if ui.button("Preparar definição").clicked() {
            panel.prepare();
        }
    });
    if let Some(definition) = &mut panel.definition {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("host de destino:");
            ui.text_edit_singleline(&mut definition.host);
            ui.label("domínio:");
            ui.text_edit_singleline(&mut definition.domain);
        });
        ui.horizontal(|ui| {
            ui.label("configuração:");
            ui.text_edit_singleline(&mut definition.config);
        });
        ui.label(format!(
            "instâncias observadas: {} agente(s) · operações: {} · evidência: {}",
            agents.list.len(),
            panel.observed_operations,
            if panel.last_evidence.is_empty() {
                "nenhuma"
            } else {
                &panel.last_evidence
            }
        ));
        ui.group(|ui| {
            ui.strong("Capacidade indisponível");
            ui.label("Aplicar a configuração exige aplicação remota versionada (G-INT-06). Revisão local preservada, nada aplicado.");
        });
    } else {
        ui.label("Nenhum monitor definido. Prepare uma definição pelo apelido.");
    }
}
