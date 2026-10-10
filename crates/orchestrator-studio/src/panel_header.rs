//! Molde de cabeçalho padrão das telas (migração Stitch; reforma UX3).
//!
//! Cada tela do Studio abre com uma FAIXA HERO: fundo elevado + filete 3px
//! ciano à esquerda, kicker mono (SEC 3.x · tema), título grande forte e
//! subtítulo técnico com o contexto DDS/QoS da fonte de dados da tela.
//! Os TEXTOS não mudam (âncoras kittest preservadas) — só o visual.

use crate::theme;
use eframe::egui;

/// Cabeçalho hero de painel (UX3): faixa com filete de acento.
pub fn panel_header(ui: &mut egui::Ui, kicker: &str, title: &str, subtitle: &str) {
    let response = egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_LG)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(kicker.to_uppercase())
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
            ui.label(
                egui::RichText::new(title)
                    .size(20.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.label(egui::RichText::new(subtitle).weak().small());
        });
    // Filete 3px ciano colado à esquerda da faixa (assinatura do design).
    let stripe = response
        .response
        .rect
        .with_max_x(response.response.rect.left() + 3.0);
    ui.painter()
        .rect_filled(stripe, 0.0, theme::PRIMARY_CONTAINER);
    ui.add_space(theme::SPACE_LG);
}

/// Chips de resumo no topo da tela (contagens da fonte da tela) — pills.
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
            let pill = egui::CornerRadius::same(theme::RADIUS_PILL as u8);
            ui.painter().rect_filled(rect, pill, theme::SURFACE_HIGH);
            ui.painter().rect_stroke(
                rect,
                pill,
                egui::Stroke::new(1.0, theme::SURFACE_HIGHEST),
                egui::StrokeKind::Inside,
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
