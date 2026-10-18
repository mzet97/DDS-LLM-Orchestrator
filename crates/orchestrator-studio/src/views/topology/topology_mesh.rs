//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn topic_active_for_tab(topic: &str, tab: u8) -> bool {
    if tab == 0 {
        topic != "TaskOutput"
    } else {
        true
    }
}

/// Contagem de tool calls nos grupos do design (Pend = PENDING; Exec =
/// ALLOWED + EXECUTING + COMPLETED; DENIED/FAILED ficam fora dos grupos).
pub(super) fn tool_breakdown(tools: &[ToolRow]) -> (usize, usize) {
    let pending = tools.iter().filter(|t| t.status == 0).count();
    let exec = tools
        .iter()
        .filter(|t| t.status == 1 || t.status == 3 || t.status == 4)
        .count();
    (pending, exec)
}

/// `true` quando alguma coleção da janela tem linha (para o selo CONVERGIDA).
pub(super) fn snapshot_has_data(snapshot: &DdsSnapshot) -> bool {
    !(snapshot.tasks.is_empty()
        && snapshot.task_outputs.is_empty()
        && snapshot.agents.is_empty()
        && snapshot.tools.is_empty()
        && snapshot.metrics.is_empty()
        && snapshot.discoveries.is_empty()
        && snapshot.studio_nodes.is_empty())
}

/// Junta ids para o subtexto dos contadores ("a · b +N").
pub(super) fn join_preview(ids: &[&str], max: usize) -> String {
    match ids.len() {
        0 => String::from("—"),
        n if n <= max => ids.join(" · "),
        n => format!("{} · +{}", ids[..max].join(" · "), n - max),
    }
}

/// Prévia de 96 chars achatada (padrão PRD para conteúdos de tarefa).
pub(super) fn preview96(text: &str) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    if flat.chars().count() <= 96 {
        flat
    } else {
        format!("{}…", flat.chars().take(96).collect::<String>())
    }
}

/// Cor do estado do ciclo de vida da tarefa.
pub(super) fn task_status_color(status: i32) -> egui::Color32 {
    match status {
        0 | 1 => theme::STALE,
        2 => theme::PRIMARY_FIXED_DIM,
        3 => theme::OK,
        4 => theme::ERROR,
        _ => theme::ON_SURFACE_VARIANT,
    }
}

// ── Mesh ─────────────────────────────────────────────────────────────────

/// Cabeçalho da seção do mesh (design: título + pill de modo + legenda).
pub(super) fn mesh_section_header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("TOPOLOGIA DE BARRAMENTO DDS EM TEMPO REAL")
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        kit::badge(ui, "MODO OBSERVADOR RTPS", theme::OUTLINE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            legend_item(ui, "Descoberta SPDP", theme::AUTH);
            legend_item(ui, "Assinante (Sub)", theme::PRIMARY_FIXED_DIM);
            legend_item(ui, "Publicador (Pub)", theme::OK);
        });
    });
    ui.add_space(theme::SPACE_XS);
}

/// Item da legenda do mesh (quadrado + rótulo; ordem RTL do chamador).
pub(super) fn legend_item(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .size(9.0)
            .color(theme::ON_SURFACE_VARIANT),
    );
    let (square, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
    ui.painter().rect_filled(square, 1.5, color);
}

/// Encaixa o texto na largura (trunca com … quando passa do limite).
pub(super) fn fit_text(
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    max_width: f32,
) -> String {
    let full = painter.layout_no_wrap(text.to_owned(), font.clone(), theme::ON_SURFACE);
    if full.size().x <= max_width {
        return text.to_owned();
    }
    let mut shown = text.to_owned();
    while shown.len() > 4 {
        shown.pop();
        let trial = painter.layout_no_wrap(format!("{shown}…"), font.clone(), theme::ON_SURFACE);
        if trial.size().x <= max_width {
            break;
        }
    }
    shown.push('…');
    shown
}

/// Desenha um card do mesh colunar no retângulo dado: faixa superior na cor
/// do tipo, título + dot, linhas de propriedades e tag de canto opcional
/// (ALVO). O chamador registra o hit-test.
#[allow(clippy::too_many_arguments)]
pub(super) fn mesh_card(
    painter: &egui::Painter,
    rect: egui::Rect,
    title: &str,
    lines: &[String],
    accent: egui::Color32,
    alive: bool,
    selected: bool,
    corner_tag: Option<&str>,
) {
    painter.rect_filled(rect, egui::CornerRadius::same(6), theme::SURFACE_CONTAINER);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(6),
        egui::Stroke::new(
            if selected { 1.75 } else { 1.0 },
            if selected {
                theme::PRIMARY_CONTAINER
            } else {
                theme::tint(accent, 40)
            },
        ),
        egui::StrokeKind::Inside,
    );
    // Faixa superior colorida por tipo (assinatura visual do design).
    painter.rect_filled(
        rect.with_max_y(rect.top() + 3.0),
        egui::CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        accent,
    );
    // Título + dot de status.
    painter.circle_filled(
        egui::pos2(rect.left() + 12.0, rect.top() + 17.0),
        3.5,
        if alive { accent } else { theme::STALE },
    );
    let mut title_max = rect.width() - 33.0;
    if let Some(tag) = corner_tag {
        let tag_text = painter.layout_no_wrap(
            tag.to_owned(),
            egui::FontId::monospace(8.0),
            theme::ON_PRIMARY,
        );
        let pill_w = tag_text.size().x + 12.0;
        let pill = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 8.0 - pill_w, rect.top() + 8.0),
            egui::vec2(pill_w, 15.0),
        );
        painter.rect_filled(pill, egui::CornerRadius::same(7), theme::PRIMARY_CONTAINER);
        painter.galley(
            egui::pos2(
                pill.center().x - tag_text.size().x / 2.0,
                pill.center().y - tag_text.size().y / 2.0,
            ),
            tag_text,
            theme::ON_PRIMARY,
        );
        title_max -= pill_w + 6.0;
    }
    painter.text(
        egui::pos2(rect.left() + 21.0, rect.top() + 9.0),
        egui::Align2::LEFT_TOP,
        fit_text(
            painter,
            title,
            egui::FontId::monospace(10.5),
            title_max.max(40.0),
        ),
        egui::FontId::monospace(10.5),
        theme::ON_SURFACE,
    );
    for (index, line) in lines.iter().enumerate() {
        painter.text(
            egui::pos2(rect.left() + 12.0, rect.top() + 28.0 + index as f32 * 15.0),
            egui::Align2::LEFT_TOP,
            fit_text(
                painter,
                line,
                egui::FontId::monospace(8.75),
                rect.width() - 24.0,
            ),
            egui::FontId::monospace(8.75),
            theme::ON_SURFACE_VARIANT,
        );
    }
}

pub(super) fn col_header(
    painter: &egui::Painter,
    inner_top: f32,
    header_font: &egui::FontId,
    left: f32,
    width: f32,
    title: &str,
    right: &str,
) {
    painter.text(
        egui::pos2(left, inner_top),
        egui::Align2::LEFT_TOP,
        fit_text(painter, title, header_font.clone(), width * 0.62),
        header_font.clone(),
        theme::OUTLINE,
    );
    if !right.is_empty() {
        painter.text(
            egui::pos2(left + width, inner_top),
            egui::Align2::RIGHT_TOP,
            right,
            header_font.clone(),
            theme::PRIMARY_FIXED_DIM,
        );
    }
}
