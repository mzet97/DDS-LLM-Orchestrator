//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

use super::topology_draw_right::draw_mesh_right;
use super::topology_mesh::col_header;

/// Mesh colunar (design 3.11): 4 grupos lado a lado — ESTAÇÃO LOCAL, NÓS
/// REMOTOS, ENXAME IA e PARTICIPANTES — com cards clicáveis (a seleção vira
/// o card "ASSINANTE SELECIONADO") e faixa de fluxos no rodapé do canvas.
/// Só dados reais: sem GUID, RTT inventado ou throughput de barramento.
pub(super) fn draw_mesh(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    let height = 410.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    let painter = ui.painter_at(rect);

    // Fundo do canvas: superfície própria + grade de pontos 16 px.
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(theme::RADIUS_MD as u8),
        theme::SURFACE_LOW,
    );
    let mut y = 10.0;
    while y < rect.height() {
        let mut x = 10.0;
        while x < rect.width() {
            painter.circle_filled(
                egui::pos2(rect.left() + x, rect.top() + y),
                1.0,
                theme::OUTLINE_VARIANT,
            );
            x += 16.0;
        }
        y += 16.0;
    }
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(theme::RADIUS_MD as u8),
        egui::Stroke::new(1.0, theme::SURFACE_HIGHEST),
        egui::StrokeKind::Inside,
    );

    // Cards clicáveis coletados para o hit-test do clique.
    let mut hit_rects: Vec<(egui::Rect, String)> = Vec::new();
    let now = now_unix_ns();

    // Geometria das 4 colunas (proporções do design).
    let pad = 12.0;
    let gap = 10.0;
    let footer_h = 30.0;
    let head_h = 20.0;
    let inner_left = rect.left() + pad;
    let inner_top = rect.top() + pad;
    let inner_width = rect.width() - pad * 2.0;
    let body_top = inner_top + head_h;
    let body_bottom = rect.bottom() - pad - footer_h;
    let cols_w = inner_width - gap * 3.0;
    let widths = [
        cols_w * 0.21,
        cols_w * 0.29,
        cols_w * 0.33,
        cols_w * 0.17 + gap * 0.0,
    ];
    let mut col_left = Vec::with_capacity(4);
    {
        let mut x = inner_left;
        for (index, width) in widths.iter().enumerate() {
            let width = if index == 3 {
                inner_left + inner_width - x
            } else {
                *width
            };
            col_left.push((x, width));
            x += width + gap;
        }
    }
    let header_font = egui::FontId::monospace(10.0);

    // ── Coluna 1: ESTAÇÃO LOCAL (este Studio, observador) ──
    col_header(
        &painter,
        inner_top,
        &header_font,
        col_left[0].0,
        col_left[0].1,
        "ESTAÇÃO LOCAL",
        "",
    );
    let drained = dds.snapshot.tasks.len()
        + dds.snapshot.task_outputs.len()
        + dds.snapshot.agents.len()
        + dds.snapshot.tools.len()
        + dds.snapshot.metrics.len()
        + dds.snapshot.discoveries.len()
        + dds.snapshot.studio_nodes.len();
    let station_rect = egui::Rect::from_min_size(
        egui::pos2(col_left[0].0, body_top),
        egui::vec2(col_left[0].1, body_bottom - body_top),
    );
    mesh_card(
        &painter,
        station_rect,
        "Studio GUI",
        &[
            format!("DOMÍNIO {} · observador", dds.domain),
            format!("DRENADOS {drained} MSG / {}s", dds.window_secs),
            String::from("LEITURA passiva (nunca take)"),
            String::from("COLEÇÕES 7 observadas"),
            String::from("MODO observador RTPS"),
        ],
        theme::PRIMARY_CONTAINER,
        true,
        dds.mesh_selected.as_deref() == Some("station"),
        None,
    );
    hit_rects.push((station_rect, String::from("station")));

    // ── Coluna 2: NÓS REMOTOS (Studio.NodePresence) ──
    let nodes: Vec<(String, bool, u64, String, bool)> = if discovery.nodes.is_empty() {
        dds.snapshot
            .studio_nodes
            .iter()
            .map(|row| (row.node_id.clone(), true, 0, row.url.clone(), false))
            .collect()
    } else {
        discovery
            .nodes
            .iter()
            .map(|node| {
                (
                    node.node_id.clone(),
                    node.is_alive(now),
                    node.age_secs(now),
                    node.url.clone(),
                    node.token_required,
                )
            })
            .collect()
    };
    let alive_nodes = nodes.iter().filter(|(_, alive, _, _, _)| *alive).count();
    col_header(
        &painter,
        inner_top,
        &header_font,
        col_left[1].0,
        col_left[1].1,
        "NÓS REMOTOS (STUDIO.NODEPRESENCE)",
        &format!("{alive_nodes} ATIVOS"),
    );
    let target_url = discovery.selected_url().unwrap_or_default();
    let mut card_top = body_top;
    for (node_id, alive, age, url, token_required) in nodes.iter().take(3) {
        let card = egui::Rect::from_min_size(
            egui::pos2(col_left[1].0, card_top),
            egui::vec2(col_left[1].1, 92.0),
        );
        let accent = if *alive { theme::OK } else { theme::STALE };
        let probe_detail = discovery
            .nodes
            .iter()
            .find(|n| n.node_id == *node_id)
            .and_then(|n| n.probe.as_ref().map(|p| p.detail.clone()));
        mesh_card(
            &painter,
            card,
            node_id,
            &[
                format!("ENDPOINT {url}"),
                format!("PROBE {}", probe_detail.as_deref().unwrap_or("sondando…")),
                format!("HB {age}s atrás · lease 10s"),
                format!(
                    "TOKEN {}",
                    if *token_required { "exigido" } else { "livre" }
                ),
            ],
            accent,
            *alive,
            dds.mesh_selected.as_deref() == Some(node_id.as_str()),
            if *url == target_url {
                Some("ALVO")
            } else {
                None
            },
        );
        hit_rects.push((card, format!("node:{node_id}")));
        card_top += 92.0 + 8.0;
    }
    if nodes.len() > 3 {
        painter.text(
            egui::pos2(col_left[1].0, card_top),
            egui::Align2::LEFT_TOP,
            format!("+{} outros na aba Instalações", nodes.len() - 3),
            egui::FontId::monospace(8.5),
            theme::OUTLINE,
        );
    }
    if nodes.is_empty() {
        painter.text(
            egui::pos2(col_left[1].0, body_top),
            egui::Align2::LEFT_TOP,
            "○ aguardando Studio.NodePresence",
            egui::FontId::monospace(9.0),
            theme::STALE,
        );
    }

    draw_mesh_right(
        painter,
        rect,
        &response,
        &col_left,
        &widths,
        inner_top,
        body_top,
        body_bottom,
        footer_h,
        header_font,
        &mut hit_rects,
        now,
        dds,
        discovery,
    );
}
