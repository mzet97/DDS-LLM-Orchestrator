//! Painel de topologia DDS: foto viva do domínio (só com feature `dds`).

use eframe::egui;
use orchestrator_studio::dds_observe::DdsState;

/// Domínio, janela, botão de observação e grades de agentes/tools/métricas.
pub fn show(ui: &mut egui::Ui, dds: &mut DdsState) {
    ui.collapsing("Topologia DDS (domínio ao vivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("domínio:");
            ui.add(egui::DragValue::new(&mut dds.domain));
            ui.label("janela (s):");
            ui.add(egui::DragValue::new(&mut dds.window_secs).range(1..=30));
            if ui.button("Observar").clicked() {
                dds.refresh();
            }
        });
        if !dds.error.is_empty() {
            ui.label(&dds.error);
        }
        ui.label(format!(
            "{} agente(s) · {} tool call(s) · {} métrica(s)",
            dds.snapshot.agents.len(),
            dds.snapshot.tools.len(),
            dds.snapshot.metrics.len()
        ));
        if !dds.snapshot.agents.is_empty() {
            egui::Grid::new("dds_agents_grid").show(ui, |ui| {
                ui.label("agent_id");
                ui.label("modelo");
                ui.label("slots");
                ui.end_row();
                for agent in &dds.snapshot.agents {
                    ui.label(&agent.agent_id);
                    ui.label(&agent.model);
                    ui.label(format!("{}/{}", agent.slots_busy, agent.slots_total));
                    ui.end_row();
                }
            });
        }
        if !dds.snapshot.tools.is_empty() {
            egui::Grid::new("dds_tools_grid").show(ui, |ui| {
                ui.label("call_id");
                ui.label("ferramenta");
                ui.label("status");
                ui.end_row();
                for tool in &dds.snapshot.tools {
                    ui.label(&tool.call_id);
                    ui.label(&tool.tool_name);
                    ui.label(tool.status.to_string());
                    ui.end_row();
                }
            });
        }
        if !dds.snapshot.metrics.is_empty() {
            egui::Grid::new("dds_metrics_grid").show(ui, |ui| {
                ui.label("origem");
                ui.label("métrica");
                ui.label("valor");
                ui.end_row();
                for metric in dds.snapshot.metrics.iter().take(20) {
                    ui.label(&metric.source);
                    ui.label(&metric.name);
                    ui.label(format!("{:.3}", metric.value));
                    ui.end_row();
                }
            });
        }
    });
}
