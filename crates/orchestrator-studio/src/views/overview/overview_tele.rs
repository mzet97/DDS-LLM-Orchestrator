//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn health_dot(health: TileHealth, stale: bool) -> egui::Color32 {
    if stale || health == TileHealth::Stale {
        return theme::STALE;
    }
    match health {
        TileHealth::Ok => theme::OK,
        TileHealth::Warn => theme::WARN,
        TileHealth::Stale => theme::STALE,
    }
}

/// Card de telemetria do design (título + status, subtítulo, linhas, rodapé).
pub(super) struct TeleCard {
    pub(super) dot: egui::Color32,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) status_color: egui::Color32,
    pub(super) subtitle: String,
    pub(super) rows: Vec<(String, String, egui::Color32)>,
    pub(super) footer: (String, String),
}

pub(super) fn tele_card(ui: &mut egui::Ui, card: &TeleCard) {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 18.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 4.0, card.dot);
                ui.label(
                    egui::RichText::new(&card.title)
                        .size(13.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&card.status)
                            .size(11.5)
                            .color(card.status_color),
                    );
                });
            });
            ui.label(
                egui::RichText::new(&card.subtitle)
                    .size(11.0)
                    .color(theme::OUTLINE),
            );
            ui.add_space(theme::SPACE_XS);
            egui::Frame::NONE
                .fill(theme::SURFACE_LOW)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    for (label, value, color) in &card.rows {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).size(11.0).color(theme::OUTLINE));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(value)
                                            .monospace()
                                            .size(11.0)
                                            .color(*color),
                                    );
                                },
                            );
                        });
                    }
                });
            ui.add_space(theme::SPACE_XS);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&card.footer.0)
                        .size(11.0)
                        .color(theme::OUTLINE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&card.footer.1)
                            .monospace()
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });
        });
}
