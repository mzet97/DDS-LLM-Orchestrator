//! Casca eframe do Studio: renderiza [`AppState`] sem inventar dados.
//!
//! Começa vazia; "Atualizar" relê o catálogo em memória e "Conectar ao nó"
//! busca o resumo vivo do `studio-noded` (T-800-08).

use anyhow::Result;
use eframe::egui;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::state::AppState;
use orchestrator_studio::workload::DispatchState;
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
use studio_core::catalog::Catalog;

struct StudioApp {
    state: AppState,
    catalog: Catalog,
    node_url: String,
    inference: InferenceState,
    agents: AgentsState,
    dispatch: DispatchState,
}

impl StudioApp {
    fn new() -> Self {
        Self {
            state: AppState::new(),
            catalog: Catalog::new(),
            node_url: String::from("http://127.0.0.1:4317"),
            inference: InferenceState::new(),
            agents: AgentsState::new(),
            dispatch: DispatchState::new(),
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(self.state.status());
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("DDS Orchestrator Studio");
            ui.horizontal(|ui| {
                ui.label("nó:");
                ui.text_edit_singleline(&mut self.node_url);
                if ui.button("Conectar ao nó").clicked() {
                    self.state.refresh_from_node(&self.node_url.clone());
                }
                if ui.button("Atualizar").clicked() {
                    let snapshot = self.catalog.snapshot();
                    self.state.refresh_from(&snapshot);
                }
            });
            if let Some(node) = self.state.node() {
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
            ui.collapsing("Inferência (servidor llama ao vivo)", |ui| {
                let infer = &mut self.inference;
                ui.horizontal(|ui| {
                    ui.label("servidor:");
                    ui.text_edit_singleline(&mut infer.server_url);
                    if ui.button("Modelos").clicked() {
                        infer.refresh_models();
                    }
                });
                if infer.models.is_empty() {
                    ui.text_edit_singleline(&mut infer.model);
                } else {
                    egui::ComboBox::from_label("modelo")
                        .selected_text(&infer.model)
                        .show_ui(ui, |ui| {
                            for candidate in infer.models.clone() {
                                ui.selectable_value(&mut infer.model, candidate.clone(), candidate);
                            }
                        });
                }
                ui.add(egui::Slider::new(&mut infer.temperature, 0.0..=2.0).text("temperatura"));
                ui.add(egui::Slider::new(&mut infer.max_tokens, 1..=4096).text("máx. tokens"));
                ui.label("prompt:");
                ui.text_edit_multiline(&mut infer.prompt);
                ui.horizontal(|ui| {
                    if ui.button("Enviar").clicked() {
                        infer.send();
                    }
                    if ui.button("Nova sessão").clicked() {
                        infer.clear_session();
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        if infer.history.is_empty() {
                            ui.label(&infer.reply);
                        }
                        for message in &infer.history {
                            let who = match message.role {
                                orchestrator_studio::inference::Role::System => "sistema",
                                orchestrator_studio::inference::Role::User => "você",
                                orchestrator_studio::inference::Role::Assistant => "assistente",
                            };
                            ui.label(format!("{who}: {}", message.content));
                        }
                    });
            });
            ui.collapsing("Agentes (orquestrador ao vivo)", |ui| {
                let agents = &mut self.agents;
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
            ui.collapsing("Despacho (tarefa real via orquestrador)", |ui| {
                let dispatch = &mut self.dispatch;
                ui.horizontal(|ui| {
                    ui.label("orquestrador:");
                    ui.text_edit_singleline(&mut dispatch.url);
                });
                ui.horizontal(|ui| {
                    ui.label("modelo:");
                    ui.text_edit_singleline(&mut dispatch.model);
                });
                ui.label("prompt:");
                ui.text_edit_multiline(&mut dispatch.prompt);
                if ui.button("Despachar e aguardar").clicked() {
                    dispatch.send();
                }
                if !dispatch.result.is_empty() {
                    ui.label(&dispatch.result);
                }
            });
            if self.state.rows().is_empty() {
                ui.label("Nenhum item no catálogo. Use \"Conectar ao nó\" para ler a origem viva.");
            } else {
                egui::Grid::new("catalog_grid").show(ui, |ui| {
                    ui.label("id");
                    ui.label("valor");
                    ui.label("revisão");
                    ui.end_row();
                    for row in self.state.rows() {
                        ui.label(&row.id);
                        ui.label(&row.value);
                        ui.label(row.revision.to_string());
                        ui.end_row();
                    }
                });
            }
        });
    }
}

fn main() -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "DDS Orchestrator Studio",
        options,
        Box::new(|_cc| Ok(Box::new(StudioApp::new()))),
    )
    .map_err(|err| anyhow::anyhow!("falha ao abrir a janela: {err}"))
}
