//! Painel de agentes: tabela viva do orquestrador.

use eframe::egui;
use orchestrator_studio::agents::AgentsState;

/// URL, botão de leitura, erro e tabela de agentes.
pub fn show(ui: &mut egui::Ui, agents: &mut AgentsState) {
    ui.collapsing("Agentes (orquestrador ao vivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("orquestrador:");
            ui.text_edit_singleline(&mut agents.url);
            if ui.button("Atualizar").clicked() {
                agents.refresh();
            }
        });
        if !agents.error.is_empty() {
            ui.label(&agents.error);
        }
        if agents.list.is_empty() {
            ui.label("Nenhum agente listado. Clique Atualizar.");
        } else {
            egui::Grid::new("agents_grid").show(ui, |ui| {
                ui.label("agent_id");
                ui.label("modelo");
                ui.label("slots");
                ui.label("concluídos");
                ui.label("falhas");
                ui.label("latência ms");
                ui.end_row();
                for agent in &agents.list {
                    ui.label(&agent.agent_id);
                    ui.label(&agent.model);
                    ui.label(format!("{}/{}", agent.slots_busy, agent.slots_total));
                    ui.label(agent.completed_total.to_string());
                    ui.label(agent.failed_total.to_string());
                    ui.label(format!("{:.1}", agent.ema_latency_ms));
                    ui.end_row();
                }
            });
        }
    });
}
