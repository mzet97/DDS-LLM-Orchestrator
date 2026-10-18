//! Parte de `app_impl` (extraída para manter os arquivos revisáveis).

use super::super::*;

impl StudioApp {
    pub(super) fn render_header(&mut self, ui: &mut egui::Ui) {
        // Header 40px (migração Stitch): título + badge de domínio + chip de
        // alvo + modo protegido. O repaint pulsante do dot é do ctx.
        egui::Panel::top("header")
            .exact_size(44.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // Logo do produto (design: flor ciano de 6 pétalas).
                    let (logo_rect, _) =
                        ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    let center = logo_rect.center();
                    for petal in 0..6 {
                        let angle = petal as f32 * std::f32::consts::PI / 3.0;
                        ui.painter().circle_filled(
                            egui::pos2(center.x + angle.cos() * 6.0, center.y + angle.sin() * 6.0),
                            2.6,
                            orchestrator_studio::theme::PRIMARY_CONTAINER,
                        );
                    }
                    ui.painter().circle_filled(
                        center,
                        2.2,
                        orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                    );
                    ui.label(
                        egui::RichText::new("DDS Orchestrator Studio")
                            .size(15.0)
                            .strong()
                            .color(orchestrator_studio::theme::ON_SURFACE),
                    );
                    ui.label(
                        egui::RichText::new(format!("v1.0-{GIT_HASH}"))
                            .monospace()
                            .size(10.0)
                            .color(orchestrator_studio::theme::OUTLINE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Avatar do operador (design: círculo claro à direita).
                        let (avatar_rect, _) =
                            ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                        let avatar = avatar_rect.center();
                        ui.painter().circle_filled(
                            avatar,
                            13.0,
                            orchestrator_studio::theme::SECONDARY_FIXED,
                        );
                        ui.painter().circle_filled(
                            egui::pos2(avatar.x, avatar.y - 4.0),
                            4.0,
                            orchestrator_studio::theme::SURFACE,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_center_size(
                                egui::pos2(avatar.x, avatar.y + 7.5),
                                egui::vec2(13.0, 5.0),
                            ),
                            2.0,
                            orchestrator_studio::theme::SURFACE,
                        );
                        // Alternador de layout (design: ícone de painel) —
                        // recolhe a sidebar para só numerações.
                        let (layout_rect, layout_toggle) =
                            ui.allocate_exact_size(egui::vec2(30.0, 26.0), egui::Sense::click());
                        ui.painter().rect_filled(
                            layout_rect,
                            egui::CornerRadius::same(6),
                            orchestrator_studio::theme::SURFACE_HIGH,
                        );
                        ui.painter().rect_stroke(
                            layout_rect,
                            egui::CornerRadius::same(6),
                            egui::Stroke::new(1.0, orchestrator_studio::theme::SURFACE_HIGHEST),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(layout_rect.left() + 6.0, layout_rect.top() + 6.0),
                                egui::vec2(6.0, 14.0),
                            ),
                            1.5,
                            orchestrator_studio::theme::PRIMARY_FIXED_DIM,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(layout_rect.left() + 15.0, layout_rect.top() + 6.0),
                                egui::vec2(9.0, 4.0),
                            ),
                            1.0,
                            orchestrator_studio::theme::OUTLINE,
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(layout_rect.left() + 15.0, layout_rect.top() + 12.0),
                                egui::vec2(9.0, 8.0),
                            ),
                            1.0,
                            orchestrator_studio::theme::SURFACE_HIGHEST,
                        );
                        if layout_toggle.clicked() {
                            self.sidebar_collapsed = !self.sidebar_collapsed;
                        }
                        if layout_toggle.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        // Modo protegido (design: pill com escudo + ação).
                        let armed = self.protected.armed;
                        let prot = header_pill(
                            ui,
                            &HeaderPill {
                                text: if armed {
                                    "MODO PROTEGIDO: ARMADO (Clique para Desarmar)"
                                } else {
                                    "MODO PROTEGIDO: DESARMADO (Clique para Armar)"
                                },
                                text_color: if armed {
                                    orchestrator_studio::theme::ERROR
                                } else {
                                    orchestrator_studio::theme::ON_SURFACE_VARIANT
                                },
                                suffix: None,
                                fill: if armed {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::ERROR,
                                        14,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGH
                                },
                                stroke: if armed {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::ERROR,
                                        60,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGHEST
                                },
                                dot_left: None,
                                dot_right: None,
                                clickable: true,
                            },
                        );
                        if prot.clicked() {
                            self.protected.set_armed(!armed);
                        }
                        if let Some(refusal) = &self.protected.last_refusal {
                            if armed {
                                ui.label(
                                    egui::RichText::new(refusal.as_str())
                                        .small()
                                        .color(orchestrator_studio::theme::ERROR),
                                );
                            }
                        }
                        // Auto-Carga (design: pill com dot ciano à direita).
                        let auto = header_pill(
                            ui,
                            &HeaderPill {
                                text: "Auto-Carga (5s)",
                                text_color: if self.auto_reload {
                                    orchestrator_studio::theme::PRIMARY_FIXED_DIM
                                } else {
                                    orchestrator_studio::theme::STALE
                                },
                                suffix: None,
                                fill: if self.auto_reload {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                                        12,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGH
                                },
                                stroke: if self.auto_reload {
                                    orchestrator_studio::theme::tint(
                                        orchestrator_studio::theme::PRIMARY_CONTAINER,
                                        55,
                                    )
                                } else {
                                    orchestrator_studio::theme::SURFACE_HIGHEST
                                },
                                dot_left: None,
                                dot_right: Some(if self.auto_reload {
                                    orchestrator_studio::theme::PRIMARY_CONTAINER
                                } else {
                                    orchestrator_studio::theme::STALE
                                }),
                                clickable: true,
                            },
                        );
                        if auto.clicked() {
                            self.auto_reload = !self.auto_reload;
                            orchestrator_studio::studio_log::info(format!(
                                "auto-carga contínua: {}",
                                if self.auto_reload {
                                    "ligada"
                                } else {
                                    "desligada"
                                }
                            ));
                        }
                        self.header_target_pills(ui);
                    });
                });
            });
    }
}
