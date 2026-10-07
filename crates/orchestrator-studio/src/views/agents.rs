//! Painel de agentes: tabela viva do orquestrador.

use crate::agents::AgentsState;
use crate::discovery::DiscoveryState;
use crate::panel_header;
use eframe::egui;

/// Agentes vivos no DOMÍNIO (DDS, descoberta automática) + tabela HTTP do
/// orquestrador (quando houver um rodando).
pub fn show(ui: &mut egui::Ui, agents: &mut AgentsState, discovery: &DiscoveryState) {
    panel_header::panel_header(
        ui,
        "SEC 3.5 · AGENTES — DDS × ORQUESTRADOR HTTP",
        "Agentes",
        "Topologia híbrida: agentes DDS em barramento peer-to-peer autônomo; telemetria HTTP do orquestrador é auxiliar",
    );
    // Agentes do AgentRegistry — sempre populados pela descoberta, sem
    // depender de orquestrador HTTP no ar.
    ui.collapsing("Agentes no domínio (DDS, ao vivo)", |ui| {
        if discovery.agents.is_empty() {
            ui.label(format!(
                "Nenhum agente com heartbeat no domínio {} — agentes aparecem aqui sozinhos ao subirem.",
                discovery.domain
            ));
        } else {
            egui::Grid::new("dds_live_agents")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("agent_id");
                    ui.strong("modelo");
                    ui.strong("slots");
                    ui.strong("latência ms");
                    ui.end_row();
                    for agent in &discovery.agents {
                        ui.monospace(&agent.agent_id);
                        ui.label(&agent.model);
                        ui.label(format!("{}/{}", agent.slots_busy, agent.slots_total));
                        ui.label(format!("{:.0}", agent.ema_latency_ms));
                        ui.end_row();
                    }
                });
        }
    });
    ui.separator();
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19, T-830-01).
    agents.poll();
    ui.collapsing("Agentes (orquestrador ao vivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("orquestrador:");
            ui.text_edit_singleline(&mut agents.url);
            ui.add_enabled_ui(!agents.busy, |ui| {
                if ui.button("Atualizar").clicked() {
                    agents.refresh();
                }
            });
        });
        if agents.busy {
            ui.label("lendo agentes…");
        }
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
