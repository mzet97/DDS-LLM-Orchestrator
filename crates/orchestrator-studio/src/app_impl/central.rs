//! Parte de `app_impl` (extraída para manter os arquivos revisáveis).

use super::super::*;

impl StudioApp {
    pub(super) fn render_central(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            // `id_salt` por tela: cada aba guarda sua posição; sem isso as
            // 14 telas compartilhariam um único offset persistente.
            let scroll_area = egui::ScrollArea::vertical().id_salt(self.section.num());
            // `STUDIO_SCROLL_Y` fixa o scroll (capturas de verificação).
            // Ausente/inválida → scroll livre: `.vertical_scroll_offset()`
            // é reaplicado TODO frame pelo egui e travaria a roda em 0.
            let fixed_y: Option<f32> = std::env::var("STUDIO_SCROLL_Y")
                .ok()
                .and_then(|raw| parse_scroll_offset(&raw));
            let scroll_area = match fixed_y {
                Some(y) => scroll_area.vertical_scroll_offset(y),
                None => scroll_area,
            };
            scroll_area.show(ui, |ui| match self.section {
                Section::Overview => {
                    let proved = self.runner.steps.len() == 5
                        && self.runner.steps.iter().all(|step| step.ok);
                    let proof = if proved {
                        self.runner
                            .steps
                            .last()
                            .map(|step| step.detail.as_str())
                            .unwrap_or("")
                    } else {
                        ""
                    };
                    orchestrator_studio::views::overview::show(
                        ui,
                        &mut self.state,
                        &mut self.services,
                        &mut self.agents,
                        &self.models,
                        &mut self.shared,
                        proof,
                        &self.discovery,
                    );
                }
                Section::Node => {
                    orchestrator_studio::views::node::show(
                        ui,
                        &mut self.state,
                        &mut self.node_url,
                        &mut self.node_token,
                        &self.discovery,
                        &self.protected,
                    );
                }
                Section::Inference => orchestrator_studio::views::inference::show(
                    ui,
                    &mut self.inference,
                    &self.discovery,
                ),
                Section::Launch => {
                    orchestrator_studio::views::launch::show(
                        ui,
                        &mut self.runner,
                        &mut self.go_chat,
                        &self.discovery,
                    );
                }
                Section::Agents => {
                    orchestrator_studio::views::agents::show(ui, &mut self.agents, &self.discovery)
                }
                Section::Models => orchestrator_studio::views::models::show(ui, &mut self.models),
                Section::Services => orchestrator_studio::views::services::show(
                    ui,
                    &mut self.services,
                    &mut self.protected,
                    &self.discovery,
                ),
                Section::Shared => {
                    orchestrator_studio::views::shared_catalog::show(ui, &mut self.shared);
                }
                Section::Machines => orchestrator_studio::views::machines::show(
                    ui,
                    &mut self.machines,
                    &mut self.discovery,
                ),
                Section::Dispatch => orchestrator_studio::views::dispatch::show(
                    ui,
                    &mut self.dispatch,
                    &self.discovery,
                ),
                #[cfg(feature = "dds")]
                Section::Topology => {
                    orchestrator_studio::views::topology::show(ui, &mut self.dds, &self.discovery)
                }
                Section::Workflow => {
                    orchestrator_studio::views::workflow::show(ui, &mut self.workflow)
                }
                #[cfg(feature = "dds")]
                Section::Tools => {
                    orchestrator_studio::views::tools::show(ui, &mut self.dds, &self.protected)
                }
                Section::Ssh => orchestrator_studio::views::ssh::show(ui, &mut self.ssh),
                Section::Logs => orchestrator_studio::views::logs::show(ui, &mut self.logs_panel),
                #[cfg(not(feature = "dds"))]
                Section::Topology => {
                    ui.heading("Topologia DDS");
                    ui.label(
                        "Recompile com --features dds para observar Tasks/TaskOutput ao vivo.",
                    );
                }
            });
            if self.go_chat {
                self.go_chat = false;
                self.section = Section::Inference;
            }
        });
    }
}
