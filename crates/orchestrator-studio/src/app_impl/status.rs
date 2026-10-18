//! Parte de `app_impl` (extraída para manter os arquivos revisáveis).

use super::super::*;

impl StudioApp {
    pub(super) fn render_status(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status")
            .exact_size(56.0)
            .resizable(false)
            .show(ui, |ui| {
                let right_w = 250.0;
                let left_w = (ui.available_width() - right_w - 12.0).max(200.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(left_w);
                        ui.set_max_width(left_w);
                        // Linha 1: estado do nó alvo (design: ● + versão + ops).
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("ESTADO DO NÓ:")
                                    .monospace()
                                    .size(10.5)
                                    .color(orchestrator_studio::theme::OUTLINE),
                            );
                            match self.state.node() {
                                Some(node) => {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "● Conectado (studio-node v{}.{} · {} ops registradas)",
                                            node.version.major,
                                            node.version.minor,
                                            node.operations.len()
                                        ))
                                        .monospace()
                                        .size(10.5)
                                        .color(orchestrator_studio::theme::PRIMARY_FIXED_DIM),
                                    );
                                }
                                None => {
                                    ui.label(
                                        egui::RichText::new(format!("○ {}", self.state.status()))
                                            .monospace()
                                            .size(10.5)
                                            .color(
                                                orchestrator_studio::theme::ON_SURFACE_VARIANT,
                                            ),
                                    );
                                }
                            }
                        });
                        // Linha 2: descoberta DDS viva (design: contagens).
                        let servers = if self.discovery.servers.is_empty() {
                            String::from("0 servidor inferência")
                        } else if self.discovery.servers.len() == 1 {
                            format!(
                                "1 servidor inferência ({})",
                                self.discovery.servers[0].server_id
                            )
                        } else {
                            format!(
                                "{} servidores inferência",
                                self.discovery.servers.len()
                            )
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "DESCOBERTA DDS: {} nós ativos (lease 10s) · {} agentes registrados · {servers}",
                                self.discovery.nodes.len(),
                                self.discovery.agents.len(),
                            ))
                            .monospace()
                            .size(10.5)
                            .color(orchestrator_studio::theme::ON_SURFACE_VARIANT),
                        );
                        // Linha 3: alvo + estado do token (só memória, nunca disco).
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "ALVO SELECIONADO: {}",
                                    self.discovery.selected_url().unwrap_or_else(|| {
                                        String::from("— nenhum alvo —")
                                    })
                                ))
                                .monospace()
                                .size(10.5)
                                .color(orchestrator_studio::theme::ON_SURFACE_VARIANT),
                            );
                            ui.label(
                                egui::RichText::new(if self.node_token.is_empty() {
                                    "[sem token em memória]"
                                } else {
                                    "[Bearer Token OK em memória]"
                                })
                                .monospace()
                                .size(10.5)
                                .color(if self.node_token.is_empty() {
                                    orchestrator_studio::theme::OUTLINE
                                } else {
                                    orchestrator_studio::theme::PRIMARY_FIXED_DIM
                                }),
                            );
                        });
                    });
                    // Bloco direito (design: latência + domínio + sem segredos;
                    // rect fixo + painter: sem ninho de layouts).
                    let ema_text = self
                        .state
                        .rtt_ema_ms()
                        .map(|ema| format!("{ema:.1}ms"))
                        .unwrap_or_else(|| String::from("—"));
                    let domain_text = self.discovery.domain.to_string();
                    let right_rows: [(&str, &str, egui::Color32); 3] = [
                        (
                            "Latência HTTP:",
                            ema_text.as_str(),
                            orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                        ),
                        (
                            "Domínio:",
                            domain_text.as_str(),
                            orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                        ),
                        (
                            "",
                            "Sem segredos em disco",
                            orchestrator_studio::theme::OUTLINE,
                        ),
                    ];
                    let (right_rect, _) =
                        ui.allocate_exact_size(egui::vec2(right_w, 54.0), egui::Sense::hover());
                    let right_font = egui::FontId::monospace(10.5);
                    for (index, (label, value, color)) in right_rows.iter().enumerate() {
                        let y = right_rect.top() + index as f32 * 18.0;
                        let value_galley = ui.painter().layout_no_wrap(
                            (*value).to_owned(),
                            right_font.clone(),
                            *color,
                        );
                        let mut x = right_rect.right() - value_galley.size().x;
                        ui.painter().galley(egui::pos2(x, y), value_galley, *color);
                        if !label.is_empty() {
                            let label_galley = ui.painter().layout_no_wrap(
                                (*label).to_owned(),
                                right_font.clone(),
                                orchestrator_studio::theme::OUTLINE,
                            );
                            x -= 6.0 + label_galley.size().x;
                            ui.painter().galley(
                                egui::pos2(x, y),
                                label_galley,
                                orchestrator_studio::theme::OUTLINE,
                            );
                        }
                    }
                });
            });
    }
}
