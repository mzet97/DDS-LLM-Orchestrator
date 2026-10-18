//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

/// Glifo do ponto por estado da sonda.
pub(super) fn dot_of(state: ProbeState) -> &'static str {
    match state {
        ProbeState::Online | ProbeState::AuthPending => "●",
        ProbeState::Offline | ProbeState::Unknown => "○",
    }
}

/// Mini-mapa mesh do domínio (card da 3.10): pontos coloridos por estado com
/// elos SPDP/SEDP pontilhados — esquemático (posições em elipse, não dados).
pub(super) fn draw_mini_mesh(
    ui: &mut egui::Ui,
    nodes: &[DiscoveredNode],
    selected: Option<usize>,
    now_ns: u64,
) {
    let height = 110.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    if nodes.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "○ sem instalações no domínio",
            egui::FontId::proportional(11.0),
            theme::OUTLINE,
        );
        return;
    }
    let shown = nodes.len().min(12);
    let center = rect.center();
    let radius = egui::vec2(rect.width() / 2.0 - 16.0, rect.height() / 2.0 - 12.0);
    let positions: Vec<egui::Pos2> = (0..shown)
        .map(|i| {
            let angle =
                -std::f32::consts::FRAC_PI_2 + i as f32 / shown as f32 * std::f32::consts::TAU;
            center + egui::vec2(angle.cos() * radius.x, angle.sin() * radius.y)
        })
        .collect();
    for i in 0..shown {
        for j in (i + 1)..shown {
            dashed_line(painter, positions[i], positions[j], theme::OUTLINE_VARIANT);
        }
    }
    for (i, pos) in positions.iter().enumerate() {
        let node = &nodes[i];
        let alive = node.is_alive(now_ns);
        let color = match (&node.probe, alive) {
            (Some(probe), _) => dot_color(probe.state),
            (None, true) => theme::PRIMARY_FIXED_DIM,
            (None, false) => theme::STALE,
        };
        if alive {
            painter.circle_stroke(*pos, 8.0, egui::Stroke::new(1.5, color));
        }
        painter.circle_filled(*pos, 4.0, color);
        if selected == Some(i) {
            painter.circle_stroke(*pos, 11.5, egui::Stroke::new(1.0, theme::ON_SURFACE));
        }
    }
    if nodes.len() > shown {
        painter.text(
            rect.right_bottom() + egui::vec2(-4.0, -2.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("+{}", nodes.len() - shown),
            egui::FontId::monospace(10.0),
            theme::OUTLINE,
        );
    }
}

/// Segmento tracejado (elo SPDP/SEDP do mini-mapa).
pub(super) fn dashed_line(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    color: egui::Color32,
) {
    let len = from.distance(to);
    if len < 1.0 {
        return;
    }
    let dir = (to - from).normalized();
    let (dash, gap) = (5.0, 4.0);
    let mut travelled = 0.0;
    while travelled < len {
        let end = (travelled + dash).min(len);
        painter.line_segment(
            [from + dir * travelled, from + dir * end],
            egui::Stroke::new(1.0, color),
        );
        travelled += dash + gap;
    }
}
