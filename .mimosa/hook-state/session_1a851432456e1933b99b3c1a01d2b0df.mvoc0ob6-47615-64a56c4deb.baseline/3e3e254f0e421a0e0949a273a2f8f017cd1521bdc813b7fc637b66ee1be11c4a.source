//! Pill do header (especificação + desenho).

use eframe::egui;

/// Especificação de um pill do header (largura medida do conteúdo — nunca
/// sobrepõe textos vizinhos, como o NÓ ALVO fazia com largura fixa).
pub(crate) struct HeaderPill<'a> {
    pub(crate) text: &'a str,
    pub(crate) text_color: egui::Color32,
    /// Segunda cor no mesmo pill (ex.: estado de conexão após a URL).
    pub(crate) suffix: Option<(&'a str, egui::Color32)>,
    pub(crate) fill: egui::Color32,
    pub(crate) stroke: egui::Color32,
    pub(crate) dot_left: Option<egui::Color32>,
    pub(crate) dot_right: Option<egui::Color32>,
    pub(crate) clickable: bool,
}

/// Desenha um pill do header e devolve a resposta (clique quando clicável).
pub(crate) fn header_pill(ui: &mut egui::Ui, spec: &HeaderPill) -> egui::Response {
    let font = egui::FontId::monospace(10.5);
    let main = ui
        .painter()
        .layout_no_wrap(spec.text.to_owned(), font.clone(), spec.text_color);
    let sub = spec
        .suffix
        .map(|(text, color)| ui.painter().layout_no_wrap(text.to_owned(), font, color));
    let mut width = 20.0 + main.size().x;
    if spec.dot_left.is_some() {
        width += 12.0;
    }
    if let Some(galley) = &sub {
        width += 8.0 + galley.size().x;
    }
    if spec.dot_right.is_some() {
        width += 12.0;
    }
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 26.0),
        if spec.clickable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let radius = egui::CornerRadius::same(orchestrator_studio::theme::RADIUS_MD as u8);
    ui.painter().rect_filled(rect, radius, spec.fill);
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, spec.stroke),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 10.0;
    if let Some(color) = spec.dot_left {
        ui.painter()
            .circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.0, color);
        x += 12.0;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - main.size().y / 2.0),
        main.clone(),
        spec.text_color,
    );
    x += main.size().x;
    if let Some(galley) = &sub {
        x += 8.0;
        ui.painter().galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley.clone(),
            spec.suffix
                .map(|(_, color)| color)
                .unwrap_or(spec.text_color),
        );
        x += galley.size().x;
    }
    if let Some(color) = spec.dot_right {
        ui.painter()
            .circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.0, color);
    }
    if spec.clickable && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}
