//! Parte de `kit` (extraída para manter os arquivos revisáveis).

use super::*;

/// Card de métrica HERO (UX3): borda 1px com acento translúcido, número
/// 32px forte no acento, label mono pequeno e subtexto — o padrão visual
/// dominante dos mockups (hierarquia pelo tamanho do número).
pub fn metric_card(
    ui: &mut egui::Ui,
    label: &str,
    value: String,
    sub: &str,
    accent: egui::Color32,
) {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_LG)
        .stroke(egui::Stroke::new(1.0, theme::tint(accent, 45)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(104.0);
            ui.label(
                egui::RichText::new(label.to_uppercase())
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.label(egui::RichText::new(value).size(42.0).strong().color(accent));
            if !sub.is_empty() {
                ui.label(
                    egui::RichText::new(sub)
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
            }
        });
}

/// Card com filete colorido à esquerda + borda de acento (UX3), largura total.
pub fn accent_card<R>(
    ui: &mut egui::Ui,
    accent: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let response = egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_LG)
        .stroke(egui::Stroke::new(1.0, theme::tint(accent, 45)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add_contents(ui)
        });
    // Filete de 3px colado na borda esquerda do card (fora do padding).
    let stripe = response
        .response
        .rect
        .with_max_x(response.response.rect.left() + 3.0);
    ui.painter()
        .rect_filled(stripe, 0.0, theme::tint(accent, 80));
    response.inner
}

/// Chip do alvo único no topo das telas HTTP (UX3: pill com borda ciana).
pub fn target_chip(ui: &mut egui::Ui, url: &str, state: &str, ok: bool) {
    let color = theme::PRIMARY_FIXED_DIM;
    let galley = ui.painter().layout_no_wrap(
        format!("ALVO: {url} · {state}"),
        egui::FontId::monospace(10.0),
        color,
    );
    let (rect, _) = ui.allocate_at_least(
        egui::vec2(galley.size().x + theme::SPACE_LG * 2.0, 24.0),
        egui::Sense::hover(),
    );
    let pill = egui::CornerRadius::same(theme::RADIUS_PILL as u8);
    ui.painter()
        .rect_filled(rect, pill, theme::tint(theme::PRIMARY_CONTAINER, 12));
    ui.painter().rect_stroke(
        rect,
        pill,
        egui::Stroke::new(1.0, theme::tint(theme::PRIMARY_CONTAINER, 55)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    if !ok {
        ui.label(
            egui::RichText::new(state)
                .monospace()
                .small()
                .color(theme::WARN),
        );
    }
}

/// Legenda de cores (linha mono com dots): `items = [(texto, cor)]`.
pub fn legend(ui: &mut egui::Ui, items: &[(&str, egui::Color32)]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("LEGENDA:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        for (text, color) in items {
            ui.label(egui::RichText::new("●").small().color(*color));
            ui.label(
                egui::RichText::new(*text)
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        }
    });
}

/// Barra de progresso textual mono com rótulo (padrão dos mockups).
pub fn progress_line(ui: &mut egui::Ui, label: &str, fraction: f32, detail: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
        );
        ui.add(
            egui::ProgressBar::new(fraction.clamp(0.0, 1.0))
                .show_percentage()
                .desired_width(220.0),
        );
        if !detail.is_empty() {
            ui.label(egui::RichText::new(detail).small().weak());
        }
    });
}
