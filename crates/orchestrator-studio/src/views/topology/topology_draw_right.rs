//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

use super::topology_mesh::col_header;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_mesh_right(
    painter: egui::Painter,
    rect: egui::Rect,
    response: &egui::Response,
    col_left: &[(f32, f32)],
    _widths: &[f32; 4],
    inner_top: f32,
    body_top: f32,
    body_bottom: f32,
    footer_h: f32,
    header_font: egui::FontId,
    hit_rects: &mut Vec<(egui::Rect, String)>,
    now: u64,
    dds: &mut DdsState,
    discovery: &DiscoveryState,
) {
    // ── Coluna 3: ENXAME IA & DISPOSITIVOS ESPECIALIZADOS ──
    col_header(
        &painter,
        inner_top,
        &header_font,
        col_left[2].0,
        col_left[2].1,
        "ENXAME IA & DISPOSITIVOS ESPECIALIZADOS",
        "",
    );
    let mut swarm_top = body_top;
    painter.text(
        egui::pos2(col_left[2].0, swarm_top),
        egui::Align2::LEFT_TOP,
        format!(
            "AGENTES DDS (AGENTREGISTRY + TASKS) · {} registrado(s)",
            discovery.agents.len()
        ),
        egui::FontId::monospace(8.5),
        theme::OUTLINE,
    );
    swarm_top += 16.0;
    if discovery.agents.is_empty() {
        painter.text(
            egui::pos2(col_left[2].0, swarm_top),
            egui::Align2::LEFT_TOP,
            "○ sem agentes no domínio",
            egui::FontId::monospace(9.0),
            theme::STALE,
        );
        swarm_top += 18.0;
    }
    for agent in discovery.agents.iter().take(2) {
        let card = egui::Rect::from_min_size(
            egui::pos2(col_left[2].0, swarm_top),
            egui::vec2(col_left[2].1, 77.0),
        );
        let age = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
        mesh_card(
            &painter,
            card,
            &agent.agent_id,
            &[
                format!("modelo {}", agent.model),
                format!(
                    "slots {}/{} · EMA {:.0} ms",
                    agent.slots_busy, agent.slots_total, agent.ema_latency_ms
                ),
                format!("HB {age}s · QoS TransientLocal"),
            ],
            theme::PRIMARY_FIXED_DIM,
            age <= 30,
            dds.mesh_selected.as_deref() == Some(agent.agent_id.as_str()),
            None,
        );
        hit_rects.push((card, format!("agent:{}", agent.agent_id)));
        swarm_top += 77.0 + 8.0;
    }
    if let Some(server) = discovery.servers.first() {
        let card = egui::Rect::from_min_size(
            egui::pos2(col_left[2].0, swarm_top),
            egui::vec2(col_left[2].1, 77.0),
        );
        mesh_card(
            &painter,
            card,
            &format!("INFERÊNCIA · {}", server.server_id),
            &[
                format!("modelo {}", server.model_loaded),
                format!(
                    "slots {}/{} · pronto {}",
                    server.slots_processing,
                    server.slots_idle + server.slots_processing,
                    if server.ready { "sim" } else { "não" }
                ),
                String::from("publica ServerStatus"),
            ],
            theme::WARN,
            server.ready,
            dds.mesh_selected.as_deref() == Some(server.server_id.as_str()),
            None,
        );
        hit_rects.push((card, format!("server:{}", server.server_id)));
        swarm_top += 77.0 + 8.0;
    }
    let tool_count = dds.snapshot.tools.len();
    let tool_pending = dds.snapshot.tools.iter().filter(|t| t.status == 0).count();
    if swarm_top + 62.0 <= body_bottom + 1.0 {
        let card = egui::Rect::from_min_size(
            egui::pos2(col_left[2].0, swarm_top),
            egui::vec2(col_left[2].1, 62.0),
        );
        mesh_card(
            &painter,
            card,
            "TOOL GATEWAY",
            &[
                format!("{tool_count} chamadas ({tool_pending} pendentes)"),
                String::from("tópico ToolCall.Request"),
            ],
            theme::PRIMARY_FIXED_DIM,
            tool_count > 0,
            dds.mesh_selected.as_deref() == Some("tools"),
            None,
        );
        hit_rects.push((card, String::from("tools")));
    }

    // ── Coluna 4: PARTICIPANTES SPDP (presença viva, curta) ──
    col_header(
        &painter,
        inner_top,
        &header_font,
        col_left[3].0,
        col_left[3].1,
        "SPDP PARTICIPANTS",
        "",
    );
    let mut row_top = body_top;
    let row_font = egui::FontId::monospace(9.0);
    for node in discovery.nodes.iter().take(4) {
        painter.text(
            egui::pos2(col_left[3].0, row_top),
            egui::Align2::LEFT_TOP,
            format!("● {} · {}s", short_host(&node.url), node.age_secs(now)),
            row_font.clone(),
            if node.is_alive(now) {
                theme::OK
            } else {
                theme::STALE
            },
        );
        row_top += 17.0;
    }
    for agent in discovery.agents.iter().take(3) {
        let age = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
        painter.text(
            egui::pos2(col_left[3].0, row_top),
            egui::Align2::LEFT_TOP,
            fit_text(
                &painter,
                &format!("● {} · {}s", short_agent(&agent.agent_id), age),
                row_font.clone(),
                col_left[3].1,
            ),
            row_font.clone(),
            if age <= 30 {
                theme::PRIMARY_FIXED_DIM
            } else {
                theme::STALE
            },
        );
        row_top += 17.0;
    }

    // ── Faixa de fluxos no rodapé do canvas ──
    let footer = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.bottom() - footer_h),
        egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
    );
    painter.rect_filled(
        footer,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 6,
            se: 6,
        },
        theme::SURFACE_HIGH,
    );
    painter.text(
        egui::pos2(footer.left() + 12.0, footer.center().y),
        egui::Align2::LEFT_CENTER,
        format!(
            "FLUXOS: Tasks {} · TaskOutput {} · ToolCall {} · Métricas {} · Descoberta {}",
            dds.snapshot.tasks.len(),
            dds.snapshot.task_outputs.len(),
            dds.snapshot.tools.len(),
            dds.snapshot.metrics.len(),
            dds.snapshot.discoveries.len()
        ),
        egui::FontId::monospace(9.0),
        theme::OUTLINE,
    );
    let converged = snapshot_has_data(&dds.snapshot);
    painter.text(
        egui::pos2(footer.right() - 12.0, footer.center().y),
        egui::Align2::RIGHT_CENTER,
        if converged {
            "✓ TOPOLOGIA CONVERGIDA (LEASES VÁLIDOS)"
        } else {
            "○ SEM AMOSTRAS NA JANELA"
        },
        egui::FontId::monospace(9.0),
        if converged { theme::OK } else { theme::STALE },
    );

    // Hit-test do clique: seleciona o card sob o ponteiro.
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some((_, key)) = hit_rects.iter().find(|(r, _)| r.contains(pos)) {
                dds.mesh_selected = Some(key.clone());
            } else {
                dds.mesh_selected = None;
            }
        }
    }
}
