//! Kit de componentes visuais compartilhados (migração Stitch → egui).
//!
//! Cada componente replica o padrão do design system: badges com tinta de
//! 10%, dots de status, banners de erro/aviso, headers de tabela em mono,
//! cards de métrica com valor grande e cards com filete de acento.
//! Sem sombra — profundidade por luminância (DESIGN.md).

use crate::theme;
use eframe::egui;

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

/// Banner de ação destrutiva pendente (PRD 3.8): vermelho — gravação no nó
/// aguardando o 2º passo; mesmo formato do `warn_banner`.
pub fn danger_banner(ui: &mut egui::Ui, message: &str) {
    let bg = theme::tint(theme::ERROR, 10);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
        .inner_margin(theme::SPACE_MD)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("⚠ {message}"))
                    .color(theme::ERROR)
                    .strong(),
            );
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

/// Célula numérica alinhada à direita (NFR do PRD v1.0: toda coluna
/// numérica das tabelas alinh à direita, mono). A largura mínima
/// reservada torna o alinhamento visível mesmo em colunas estreitas.
pub fn num_cell(ui: &mut egui::Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_min_width(72.0);
        ui.label(
            egui::RichText::new(text)
                .monospace()
                .color(theme::ON_SURFACE_VARIANT),
        );
    });
}

/// Rótulo de seção interna da tela (ex.: "SEÇÃO A — FONTE 1: DDS"):
/// mono uppercase em OUTLINE com espaçamento.
pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .monospace()
            .small()
            .color(theme::OUTLINE),
    );
    ui.add_space(theme::SPACE_XS);
}

/// Card de métrica (mockups 3.11/3.7/3.14): rótulo mono uppercase, valor
/// grande no accent e subtexto real; ocupa a largura disponível da coluna.
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
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(label.to_uppercase())
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.label(egui::RichText::new(value).size(22.0).strong().color(accent));
            if !sub.is_empty() {
                ui.label(
                    egui::RichText::new(sub)
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
            }
        });
}

/// Card com filete colorido à esquerda (estado por cor), largura total.
pub fn accent_card<R>(
    ui: &mut egui::Ui,
    accent: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let response = egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add_contents(ui)
        });
    // Filete de 3px colado na borda esquerda do card (fora do padding).
    let stripe = response
        .response
        .rect
        .with_max_x(response.response.rect.left() + 3.0);
    ui.painter().rect_filled(stripe, 0.0, accent);
    response.inner
}

/// Chip do alvo único no topo das telas HTTP: "ALVO: url · estado".
pub fn target_chip(ui: &mut egui::Ui, url: &str, state: &str, ok: bool) {
    let galley = ui.painter().layout_no_wrap(
        format!("ALVO: {url} · {state}"),
        egui::FontId::monospace(10.0),
        theme::PRIMARY_FIXED_DIM,
    );
    let (rect, _) = ui.allocate_at_least(
        egui::vec2(galley.size().x + theme::SPACE_LG * 2.0, 22.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(theme::RADIUS_SM as u8),
        theme::tint(theme::PRIMARY_CONTAINER, 10),
    );
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        theme::PRIMARY_FIXED_DIM,
    );
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
