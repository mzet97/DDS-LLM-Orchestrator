//! Parte de `app_impl` (extraída para manter os arquivos revisáveis).

use super::super::*;

impl StudioApp {
    pub(super) fn header_target_pills(&mut self, ui: &mut egui::Ui) {
        // NÓ ALVO (design: URL + estado + ▾ num pill só).
        if let Some(target) = self.discovery.selected_url() {
            let connected = self.state.node().is_some();
            let state_text = if connected {
                format!(
                    "● Conectado v{}.{} ▾",
                    self.state.node().map(|n| n.version.major).unwrap_or(1),
                    self.state.node().map(|n| n.version.minor).unwrap_or(0)
                )
            } else {
                String::from("○ conectando…")
            };
            header_pill(
                ui,
                &HeaderPill {
                    text: &format!("NÓ ALVO: {target}"),
                    text_color: orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                    suffix: Some((
                        &state_text,
                        if connected {
                            orchestrator_studio::theme::OK
                        } else {
                            orchestrator_studio::theme::WARN
                        },
                    )),
                    fill: orchestrator_studio::theme::tint(
                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                        12,
                    ),
                    stroke: orchestrator_studio::theme::tint(
                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                        55,
                    ),
                    dot_left: None,
                    dot_right: None,
                    clickable: false,
                },
            );
        }
        // Domínio (design: pill ciano com dot pulsante).
        let any_alive = self
            .discovery
            .nodes
            .iter()
            .any(|n| n.is_alive(orchestrator_studio::machines::now_unix_ns()));
        let pulse: f32 = if any_alive {
            (ui.input(|i| i.time) * 2.0).sin() as f32 * 0.5 + 0.5
        } else {
            1.0
        };
        header_pill(
            ui,
            &HeaderPill {
                text: &format!("DDS DOMAIN {}", self.discovery.domain),
                text_color: orchestrator_studio::theme::ON_SURFACE_VARIANT,
                suffix: None,
                fill: orchestrator_studio::theme::tint(
                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                    10,
                ),
                stroke: orchestrator_studio::theme::tint(
                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                    45,
                ),
                dot_left: Some(
                    orchestrator_studio::theme::PRIMARY_CONTAINER.gamma_multiply(1.0 - pulse * 0.5),
                ),
                dot_right: None,
                clickable: false,
            },
        );
    }
}
