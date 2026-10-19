//! Parte de `kit` (extraída para manter os arquivos revisáveis).

use super::*;

/// Dot de status pulsante (o repaint é responsabilidade do chamador).
pub fn status_dot(ui: &mut egui::Ui, color: egui::Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), size / 2.0, color);
}

/// Badge PILL (UX3): fundo tint 18% + borda 1px da cor + texto mono 10px.
pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let bg = theme::tint(color, 18);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), egui::FontId::monospace(10.0), color);
    let (rect, _) = ui.allocate_at_least(
        egui::vec2(galley.size().x + theme::SPACE_LG * 2.0, 20.0),
        egui::Sense::hover(),
    );
    let pill = egui::CornerRadius::same(theme::RADIUS_PILL as u8);
    ui.painter().rect_filled(rect, pill, bg);
    ui.painter().rect_stroke(
        rect,
        pill,
        egui::Stroke::new(1.0, theme::tint(color, 60)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// Botão primário PILL (UX3): preenchido ciano, texto ON_PRIMARY mono forte.
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .strong()
                .color(theme::ON_PRIMARY),
        )
        .fill(theme::PRIMARY_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_PILL as u8)),
    )
}

/// Chip selecionável PILL (UX3 — filtros/abas/janelas): ativo = tinta 18% +
/// borda da cor; inativo = superfície HIGH discreta com hairline.
pub fn pill_chip(
    ui: &mut egui::Ui,
    active: bool,
    text: &str,
    color: egui::Color32,
) -> egui::Response {
    let fill = if active {
        theme::tint(color, 18)
    } else {
        theme::SURFACE_HIGH
    };
    let stroke_color = if active {
        theme::tint(color, 70)
    } else {
        theme::SURFACE_HIGHEST
    };
    let text_color = if active {
        color
    } else {
        theme::ON_SURFACE_VARIANT
    };
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .small()
                .color(text_color),
        )
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_PILL as u8)),
    )
}

/// Banner de erro: fundo tinta de erro + texto colorido.
pub fn error_banner(ui: &mut egui::Ui, message: &str) {
    let bg = theme::tint(theme::ERROR, 12);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::tint(theme::ERROR, 50)))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(format!("⚠ {message}")).color(theme::ERROR));
        });
}

/// Banner de confirmação em 2 passos (modo protegido): fundo âmbar.
pub fn warn_banner(ui: &mut egui::Ui, message: &str) {
    let bg = theme::tint(theme::WARN, 12);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::tint(theme::WARN, 50)))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(format!("⚠ {message}")).color(theme::WARN));
        });
}

/// Banner de ação destrutiva pendente (PRD 3.8): vermelho — gravação no nó
/// aguardando o 2º passo; mesmo formato do `warn_banner`.
pub fn danger_banner(ui: &mut egui::Ui, message: &str) {
    let bg = theme::tint(theme::ERROR, 14);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::tint(theme::ERROR, 60)))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("⚠ {message}"))
                    .color(theme::ERROR)
                    .strong(),
            );
        });
}

/// Estado vazio honesto: ícone ○ + instrução.
pub fn empty_state(ui: &mut egui::Ui, message: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("○").color(theme::STALE).size(15.0));
        ui.label(egui::RichText::new(message).weak());
    });
}

/// Header de tabela (UX3): cada coluna vira uma mini-faixa preenchida
/// (SURFACE_HIGH, radius topo) com texto mono uppercase — o texto é o MESMO
/// de antes (âncoras kittest preservadas), só o visual muda.
pub fn grid_header(ui: &mut egui::Ui, columns: &[&str]) {
    for column in columns {
        egui::Frame::NONE
            .fill(theme::SURFACE_HIGH)
            .corner_radius(egui::CornerRadius {
                nw: theme::RADIUS_SM as u8,
                ne: theme::RADIUS_SM as u8,
                sw: 0,
                se: 0,
            })
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(column.to_uppercase())
                        .monospace()
                        .size(11.0)
                        .strong()
                        .color(theme::ON_SURFACE_VARIANT),
                );
            });
    }
    ui.end_row();
}

/// Grid de tabela no padrão UX3: zebra + linhas altas (28px) + respiro
/// entre colunas. Troca `egui::Grid::new(id).striped(true)` nas views.
pub fn table(id: &str) -> egui::Grid {
    egui::Grid::new(id)
        .striped(true)
        .min_row_height(28.0)
        .spacing(egui::vec2(14.0, 6.0))
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

/// Rótulo de seção interna da tela (UX3): mono uppercase em OUTLINE com
/// hairline na largura total abaixo (separador de seção dos mockups).
pub fn section_label(ui: &mut egui::Ui, text: &str) {
    let label = egui::RichText::new(text.to_uppercase())
        .monospace()
        .small()
        .color(theme::OUTLINE);
    let response = ui.add_sized([ui.available_width(), 16.0], egui::Label::new(label));
    let y = response.rect.bottom() + 2.0;
    ui.painter().line_segment(
        [
            egui::pos2(response.rect.left(), y),
            egui::pos2(response.rect.right(), y),
        ],
        egui::Stroke::new(1.0, theme::SURFACE_HIGHEST),
    );
    ui.add_space(theme::SPACE_SM);
}
