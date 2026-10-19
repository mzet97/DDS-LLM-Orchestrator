//! Parte de `views::services` (extraída para manter os arquivos revisáveis).

use super::*;

/// Botão de comando: destaque ciano quando é a correção sugerida.
pub(super) fn cmd_button(highlight: bool, text: &str) -> egui::Button<'_> {
    let label = if highlight {
        egui::RichText::new(text)
            .monospace()
            .small()
            .color(theme::ON_PRIMARY)
            .strong()
    } else {
        egui::RichText::new(text).monospace().small()
    };
    let mut button = egui::Button::new(label);
    if highlight {
        button = button
            .fill(theme::PRIMARY_CONTAINER)
            .stroke(egui::Stroke::NONE);
    }
    button
}

pub(super) fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}
