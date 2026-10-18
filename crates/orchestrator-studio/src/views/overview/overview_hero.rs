//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

/// Card de descoberta do hero (kicker + número + nome + linhas mono).
pub(super) struct HeroCard {
    pub(super) kicker: String,
    pub(super) count: String,
    pub(super) name: String,
    pub(super) lines: Vec<(String, String)>,
}

pub(super) fn hero_card(ui: &mut egui::Ui, card: &HeroCard) {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(&card.kicker)
                    .monospace()
                    .size(10.5)
                    .color(theme::OUTLINE),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&card.count)
                        .size(30.0)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(&card.name)
                        .size(15.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
            });
            for (left, right) in &card.lines {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(left)
                            .monospace()
                            .size(10.5)
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(right)
                                .monospace()
                                .size(10.5)
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                    });
                });
            }
        });
}
