//! Molde de cabeçalho padrão das telas (migração Stitch 2ª rodada).
//!
//! Cada tela do Studio abre com: kicker mono (SEC 3.x · tema), título e
//! subtítulo técnico com o contexto DDS/QoS da fonte de dados da tela.

use crate::theme;
use eframe::egui;

/// Cabeçalho padrão de painel: kicker + título + subtítulo técnico.
pub fn panel_header(ui: &mut egui::Ui, kicker: &str, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new(kicker.to_uppercase())
            .monospace()
            .small()
            .color(theme::PRIMARY_FIXED_DIM),
    );
    ui.heading(title);
    ui.label(egui::RichText::new(subtitle).weak().small());
    ui.add_space(theme::SPACE_MD);
}

/// Chips de resumo no topo da tela (contagens da fonte da tela).
pub fn summary_chips(ui: &mut egui::Ui, chips: &[(&str, String)]) {
    ui.horizontal_wrapped(|ui| {
        for (label, value) in chips {
            let text = format!("{label}: {value}");
            let galley = ui.painter().layout_no_wrap(
                text.clone(),
                egui::FontId::monospace(10.0),
                theme::ON_SURFACE_VARIANT,
            );
            let (rect, _) = ui.allocate_at_least(
                egui::vec2(galley.size().x + theme::SPACE_LG * 2.0, 22.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(theme::RADIUS_SM as u8),
                theme::tint(theme::SURFACE_HIGH, 100),
            );
            ui.painter().galley(
                rect.center() - galley.size() / 2.0,
                galley,
                theme::ON_SURFACE_VARIANT,
            );
        }
    });
    ui.add_space(theme::SPACE_SM);
}
