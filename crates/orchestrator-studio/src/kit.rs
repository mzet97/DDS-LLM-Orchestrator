//! Kit de componentes visuais compartilhados (migração Stitch → egui).
//!
//! Cada componente replica o padrão do design system: badges com tinta de
//! 10%, dots de status, banners de erro/aviso, headers de tabela em mono.
//! Sem sombra — profundidade por luminância (DESIGN.md).

use eframe::egui;

use crate::theme;

/// Dot de status pulsante (o repaint é responsabilidade do chamador).
pub fn status_dot(ui: &mut egui::Ui, color: egui::Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), size / 2.0, color);
}

/// Badge: tinta de 10% no fundo + texto colorido + radius 2.
pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let bg = theme::tint(color, 10);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), egui::FontId::monospace(10.0), color);
    let (rect, _) = ui.allocate_at_least(
        egui::vec2(galley.size().x + theme::SPACE_MD * 2.0, 16.0),
        egui::Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(theme::RADIUS_SM as u8), bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// Banner de erro: fundo tinta de erro + texto colorido.
pub fn error_banner(ui: &mut egui::Ui, message: &str) {
    let bg = theme::tint(theme::ERROR, 10);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
        .inner_margin(theme::SPACE_MD)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(format!("⚠ {message}")).color(theme::ERROR));
        });
}

/// Banner de confirmação em 2 passos (modo protegido): fundo âmbar.
pub fn warn_banner(ui: &mut egui::Ui, message: &str) {
    let bg = theme::tint(theme::WARN, 10);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
        .inner_margin(theme::SPACE_MD)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(format!("⚠ {message}")).color(theme::WARN));
        });
}

/// Estado vazio honesto: ícone ◌ + instrução.
pub fn empty_state(ui: &mut egui::Ui, message: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("◌").color(theme::STALE).size(14.0));
        ui.label(egui::RichText::new(message).weak());
    });
}

/// Header de tabela (grid): fundo HIGH, texto mono uppercase em OUTLINE.
pub fn grid_header(ui: &mut egui::Ui, columns: &[&str]) {
    for column in columns {
        ui.label(
            egui::RichText::new(column.to_uppercase())
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
    }
    ui.end_row();
}

/// Célula monospace para IDs/hex/URLs.
pub fn mono_cell(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .color(theme::ON_SURFACE_VARIANT),
    );
}
