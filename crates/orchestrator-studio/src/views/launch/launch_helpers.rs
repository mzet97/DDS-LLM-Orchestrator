//! Parte de `views::launch` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

/// Cabeçalho de card: título à esquerda + etiqueta à direita.
pub(super) fn card_title(ui: &mut egui::Ui, title: &str, tag: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tag)
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
}

/// Campo numérico com parse honesto (inválido mantém o anterior).
pub(super) fn num_field(ui: &mut egui::Ui, value: &mut u32, label: &str) -> bool {
    let mut raw = value.to_string();
    ui.label(
        egui::RichText::new(label)
            .monospace()
            .small()
            .color(theme::OUTLINE),
    );
    let changed = ui
        .add(egui::TextEdit::singleline(&mut raw).desired_width(90.0))
        .changed();
    if changed {
        if let Ok(parsed) = raw.trim().parse::<u32>() {
            *value = parsed;
        }
        return true;
    }
    false
}
