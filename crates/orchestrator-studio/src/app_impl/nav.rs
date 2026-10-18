//! Parte de `app_impl` (extraída para manter os arquivos revisáveis).

use super::super::*;

impl StudioApp {
    pub(super) fn render_nav(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("nav")
            .exact_size(if self.sidebar_collapsed { 52.0 } else { 248.0 })
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(orchestrator_studio::theme::SPACE_SM);
                for (group, group_tag, sections) in NAV_GROUPS {
                    if self.sidebar_collapsed {
                        // Recolhida: só numerações clicáveis.
                        for section in *sections {
                            let selected = self.section == *section;
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 26.0),
                                egui::Sense::click(),
                            );
                            if selected {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(4),
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                );
                            }
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                section.num(),
                                egui::FontId::monospace(9.5),
                                if selected {
                                    orchestrator_studio::theme::ON_PRIMARY
                                } else {
                                    orchestrator_studio::theme::OUTLINE
                                },
                            );
                            if response.clicked() {
                                self.section = *section;
                            }
                        }
                        ui.add_space(orchestrator_studio::theme::SPACE_SM);
                        continue;
                    }
                    // Cabeçalho do grupo: rótulo + tag pill à direita.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new((*group).to_uppercase())
                                .monospace()
                                .small()
                                .color(orchestrator_studio::theme::OUTLINE),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tag = ui.painter().layout_no_wrap(
                                (*group_tag).to_owned(),
                                egui::FontId::monospace(9.0),
                                orchestrator_studio::theme::OUTLINE,
                            );
                            let (tag_rect, _) = ui.allocate_exact_size(
                                egui::vec2(tag.size().x + 12.0, 15.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                tag_rect,
                                egui::CornerRadius::same(4),
                                orchestrator_studio::theme::SURFACE_HIGH,
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    tag_rect.center().x - tag.size().x / 2.0,
                                    tag_rect.center().y - tag.size().y / 2.0,
                                ),
                                tag,
                                orchestrator_studio::theme::OUTLINE,
                            );
                        });
                    });
                    ui.add_space(orchestrator_studio::theme::SPACE_XS);
                    for section in *sections {
                        let busy = matches!(section, Section::Models) && self.models.is_busy();
                        let base = section.label();
                        let label = if busy {
                            format!("{base} …")
                        } else {
                            base.to_owned()
                        };
                        let selected = self.section == *section;
                        // Linha desenhada à mão (design: selecionado = ciano
                        // sólido com texto escuro + tag pill à direita).
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 30.0),
                            egui::Sense::click(),
                        );
                        if selected {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(6),
                                orchestrator_studio::theme::PRIMARY_CONTAINER,
                            );
                        } else if response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(6),
                                orchestrator_studio::theme::tint(
                                    orchestrator_studio::theme::PRIMARY_CONTAINER,
                                    8,
                                ),
                            );
                        }
                        let num_font = egui::FontId::monospace(10.5);
                        let num_size = ui
                            .painter()
                            .layout_no_wrap(
                                section.num().to_owned(),
                                num_font.clone(),
                                orchestrator_studio::theme::OUTLINE,
                            )
                            .size()
                            .x;
                        ui.painter().text(
                            egui::pos2(rect.left() + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            section.num(),
                            num_font,
                            if selected {
                                orchestrator_studio::theme::ON_PRIMARY
                            } else {
                                orchestrator_studio::theme::OUTLINE
                            },
                        );
                        // Tag pill à direita (mede antes para truncar o rótulo).
                        let tag_font = egui::FontId::monospace(8.5);
                        let tag_text = ui.painter().layout_no_wrap(
                            section.tag().to_owned(),
                            tag_font.clone(),
                            orchestrator_studio::theme::OUTLINE,
                        );
                        let pill_w = tag_text.size().x + 12.0;
                        let pill_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.right() - 8.0 - pill_w, rect.center().y - 8.0),
                            egui::vec2(pill_w, 16.0),
                        );
                        ui.painter().rect_filled(
                            pill_rect,
                            egui::CornerRadius::same(8),
                            if selected {
                                // Ciano escuro sólido: legível sobre o ciano cheio.
                                egui::Color32::from_rgb(0x00, 0x7a, 0x8a)
                            } else {
                                orchestrator_studio::theme::SURFACE_HIGH
                            },
                        );
                        ui.painter().galley(
                            egui::pos2(
                                pill_rect.center().x - tag_text.size().x / 2.0,
                                pill_rect.center().y - tag_text.size().y / 2.0,
                            ),
                            tag_text,
                            if selected {
                                orchestrator_studio::theme::ON_PRIMARY
                            } else {
                                orchestrator_studio::theme::OUTLINE
                            },
                        );
                        // Rótulo truncado no espaço restante.
                        let label_x = rect.left() + 10.0 + num_size + 8.0;
                        let label_max = pill_rect.left() - 6.0 - label_x;
                        let label_font = egui::FontId::proportional(11.5);
                        let mut shown = label.clone();
                        while ui
                            .painter()
                            .layout_no_wrap(
                                shown.clone(),
                                label_font.clone(),
                                orchestrator_studio::theme::ON_SURFACE,
                            )
                            .size()
                            .x
                            > label_max.max(20.0)
                            && shown.len() > 4
                        {
                            shown.pop();
                        }
                        if shown.len() != label.len() {
                            shown.push('…');
                        }
                        ui.painter().text(
                            egui::pos2(label_x, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            shown,
                            label_font,
                            if selected {
                                orchestrator_studio::theme::ON_PRIMARY
                            } else {
                                orchestrator_studio::theme::ON_SURFACE
                            },
                        );
                        if response.clicked() {
                            self.section = *section;
                        }
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                    ui.add_space(orchestrator_studio::theme::SPACE_MD);
                }
            });
    }
}
