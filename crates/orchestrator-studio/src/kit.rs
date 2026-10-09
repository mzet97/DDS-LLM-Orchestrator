//! Kit de componentes visuais compartilhados (migração Stitch → egui;
//! reforma visual UX3: hierarquia tipográfica forte, pills com borda,
//! cabeçalho de tabela em faixa, cards com borda de acento).
//! Sem sombra — profundidade por luminância (DESIGN.md).

use crate::theme;
use eframe::egui;

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

/// Host curto do design (".61" de "http://192.168.1.61:4317"); fora de IPv4
/// cai para a URL truncada.
pub fn short_host(url: &str) -> String {
    let authority = url.rsplit("://").next().unwrap_or(url);
    let host = authority.split(':').next().unwrap_or(authority);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|part| part.parse::<u8>().is_ok()) {
        format!(".{}", parts[3])
    } else {
        url.chars().take(12).collect()
    }
}

/// Autoridade curta (host:port sem esquema nem caminho).
pub fn authority(url: &str) -> String {
    url.rsplit("://")
        .next()
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .chars()
        .take(24)
        .collect()
}

/// Agente curto do design ("lab-01" de "agent-lab-01").
pub fn short_agent(agent_id: &str) -> String {
    agent_id
        .strip_prefix("agent-")
        .unwrap_or(agent_id)
        .chars()
        .take(14)
        .collect()
}

/// ms unix → HH:MM:SS.mmm (relógios de auditoria/sync; conta manual).
#[must_use]
pub fn clock_ms(ts_unix_ms: u64) -> String {
    let secs = ts_unix_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        ts_unix_ms % 1000,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_ms_matches_design() {
        assert_eq!(clock_ms(0), "00:00:00.000");
        assert_eq!(clock_ms(50_569_102), "14:02:49.102");
    }
}
