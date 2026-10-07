//! Painel 3.11 Topologia DDS & Enxame Distribuído (tela inicial do Studio).
//!
//! Refeito sobre o mockup Stitch: barra de kicker com QoS do contrato,
//! janela em chips, faixa de 6 contadores, mesh desenhado com `Painter`
//! (estação local no centro, nós ao redor, enxame IA e inferência como
//! satélites) e painel de 5 abas com as coleções drenadas + filtro.
//! Só dados reais: nada de GUID/RTT/throughput inventado.

use crate::dds_observe::{DdsState, DdsSnapshot};
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::{now_unix_ns, ProbeState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Presets de janela do mockup (chips 1/5/10/15/30 s).
const WINDOW_PRESETS: [u64; 5] = [1, 5, 10, 15, 30];

/// Abas do painel de coleções (índice = `DdsState::tab`).
const TABS: [&str; 5] = [
    "Agentes DDS",
    "Tool Calls",
    "Métricas",
    "Descoberta DDS",
    "Instalações",
];

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    // Drena o worker de observação (thread + mpsc — REQ/T-820-19).
    dds.poll();
    panel_header(
        ui,
        &format!(
            "SEC 3.11 · TOPOLOGIA DE REDE DDS · DOMÍNIO {}",
            dds.domain
        ),
        "Topologia DDS & Enxame Distribuído",
        "Observação passiva (ownership 0, leitura — nunca take) · QoS do contrato: \
         Reliable + Transient Local nos tópicos de presença",
    );

    // ── Barra de controles: domínio + janela em chips + Observar ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("DOMÍNIO:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        ui.add(egui::DragValue::new(&mut dds.domain).range(0..=230));
        ui.label(
            egui::RichText::new("JANELA DDS:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        for secs in WINDOW_PRESETS {
            let active = dds.window_secs == secs;
            if ui
                .selectable_label(
                    active,
                    egui::RichText::new(format!("{secs} s")).monospace().small(),
                )
                .clicked()
            {
                dds.window_secs = secs;
            }
        }
        ui.add_enabled_ui(!dds.busy, |ui| {
            let observe = ui.add(
                egui::Button::new(
                    egui::RichText::new(if dds.busy {
                        "Observando domínio…"
                    } else {
                        "Observar Domínio"
                    })
                    .monospace()
                    .small()
                    .color(theme::ON_PRIMARY),
                )
                .fill(theme::PRIMARY_CONTAINER),
            );
            if observe.clicked() {
                crate::studio_log::info(format!(
                    "topologia: observação manual do domínio {} (janela {}s)",
                    dds.domain, dds.window_secs
                ));
                dds.refresh();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    // Estado da observação como badge (mockup: ONLINE / observando).
    ui.horizontal(|ui| {
        if dds.busy {
            kit::badge(ui, "● OBSERVANDO…", theme::WARN);
        } else if !dds.error.is_empty() {
            kit::badge(ui, "● FALHA NA OBSERVAÇÃO", theme::ERROR);
        } else if snapshot_has_data(&dds.snapshot) {
            kit::badge(ui, "● TOPOLOGIA CONVERGIDA", theme::OK);
        } else {
            kit::badge(ui, "◌ SEM AMOSTRAS NA JANELA", theme::STALE);
        }
    });
    if !dds.error.is_empty() && !dds.busy {
        kit::error_banner(ui, &dds.error);
    }
    ui.add_space(theme::SPACE_MD);

    // ── Faixa de 6 contadores (todos reais: coleções + descoberta) ──
    let snapshot = &dds.snapshot;
    let drained = snapshot.agents.len()
        + snapshot.tools.len()
        + snapshot.metrics.len()
        + snapshot.discoveries.len()
        + snapshot.studio_nodes.len();
    let node_ids: Vec<&str> = discovery.nodes.iter().map(|n| n.node_id.as_str()).collect();
    let agent_ids: Vec<&str> = discovery.agents.iter().map(|a| a.agent_id.as_str()).collect();
    let first_server = discovery
        .servers
        .first()
        .map(|s| s.server_id.as_str())
        .unwrap_or("—");
    ui.columns(6, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Eventos na janela",
            drained.to_string(),
            &format!("drenados em {} s (leitura)", dds.window_secs),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Nós Studio",
            discovery.nodes.len().to_string(),
            &join_preview(&node_ids, 2),
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Agentes DDS",
            discovery.agents.len().to_string(),
            &join_preview(&agent_ids, 2),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[3],
            "Inferência",
            discovery.servers.len().to_string(),
            first_server,
            theme::WARN,
        );
        kit::metric_card(
            &mut cols[4],
            "Tool Calls",
            snapshot.tools.len().to_string(),
            &format!("janela de {} s", dds.window_secs),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[5],
            "Métricas",
            snapshot.metrics.len().to_string(),
            &format!("janela de {} s", dds.window_secs),
            theme::PRIMARY_FIXED_DIM,
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Mesh (Painter): estação local no centro + nós + enxame + inferência ──
    draw_mesh(ui, dds, discovery);
    ui.label(
        egui::RichText::new(
            "TÓPICOS DO CONTRATO (19 canônicos): Tasks · TaskOutput · AgentRegistry · \
             ServerStatus · Studio.NodePresence · LLM.InferenceRequest/Result · \
             ToolCall.Request · SystemMetrics · Execution.Trace",
        )
        .monospace()
        .small()
        .color(theme::OUTLINE),
    );
    ui.add_space(theme::SPACE_MD);

    // ── Painel de 5 abas com filtro ──
    ui.horizontal(|ui| {
        for (index, name) in TABS.iter().enumerate() {
            let active = dds.tab as usize == index;
            if ui
                .selectable_label(
                    active,
                    egui::RichText::new(*name).monospace().small(),
                )
                .clicked()
            {
                dds.tab = index as u8;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut dds.filter)
                    .hint_text("filtrar por substring (id, tópico, entidade…)")
                    .desired_width(240.0)
                    .font(egui::TextStyle::Monospace),
            );
        });
    });
    ui.add_space(theme::SPACE_SM);
    let filter = dds.filter.trim().to_lowercase();
    let matches = |parts: &[&str]| {
        filter.is_empty()
            || parts
                .iter()
                .any(|part| part.to_lowercase().contains(&filter))
    };
    match dds.tab {
        1 => tools_tab(ui, &dds.snapshot, &filter, matches),
        2 => metrics_tab(ui, &dds.snapshot, &filter, matches),
        3 => discovery_tab(ui, &dds.snapshot, &filter, matches),
        4 => installations_tab(ui, discovery),
        _ => agents_tab(ui, discovery),
    }
}

/// `true` quando alguma coleção da janela tem linha (para o selo CONVERGIDA).
fn snapshot_has_data(snapshot: &DdsSnapshot) -> bool {
    !(snapshot.agents.is_empty()
        && snapshot.tools.is_empty()
        && snapshot.metrics.is_empty()
        && snapshot.discoveries.is_empty()
        && snapshot.studio_nodes.is_empty())
}

/// Junta ids para o subtexto dos contadores ("a · b +N").
fn join_preview(ids: &[&str], max: usize) -> String {
    match ids.len() {
        0 => String::from("—"),
        n if n <= max => ids.join(" · "),
        n => format!("{} · +{}", ids[..max].join(" · "), n - max),
    }
}

// ── Mesh ─────────────────────────────────────────────────────────────────

/// Desenha o diagrama de topologia: grade de pontos, estação local no
/// centro, nós Studio no arco superior, enxame IA à direita e servidor de
/// inferência à esquerda — linhas rotuladas pelos tópicos reais.
fn draw_mesh(ui: &mut egui::Ui, dds: &DdsState, discovery: &DiscoveryState) {
    let height = 300.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    // Fundo: grade de pontos 16 px (textura do mockup, sem gradiente).
    let mut y = 8.0;
    while y < rect.height() {
        let mut x = 8.0;
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

    // Título do canvas (mockup: "MODO OBSERVADOR").
    painter.text(
        egui::pos2(rect.left() + theme::SPACE_LG, rect.top() + theme::SPACE_LG),
        egui::Align2::LEFT_TOP,
        "TOPOLOGIA DE BARRAMENTO DDS · MODO OBSERVADOR",
        egui::FontId::monospace(10.0),
        theme::OUTLINE,
    );

    let now = now_unix_ns();
    let center = rect.center();

    // Estação local (este Studio) no centro.
    let local_box = egui::Rect::from_center_size(center, egui::vec2(160.0, 48.0));
    painter.rect_filled(local_box, egui::CornerRadius::same(4), theme::SURFACE_HIGH);
    painter.rect_stroke(
        local_box,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0, theme::PRIMARY_CONTAINER),
        egui::StrokeKind::Inside,
    );
    painter.text(
        local_box.left_top() + egui::vec2(8.0, 8.0),
        egui::Align2::LEFT_TOP,
        "ESTAÇÃO LOCAL",
        egui::FontId::monospace(10.0),
        theme::PRIMARY_FIXED_DIM,
    );
    painter.text(
        local_box.left_top() + egui::vec2(8.0, 24.0),
        egui::Align2::LEFT_TOP,
        "STUDIO GUI (esta máquina)",
        egui::FontId::monospace(9.0),
        theme::ON_SURFACE_VARIANT,
    );

    // Nós Studio no arco superior (presença contínua da descoberta; fallback
    // para a foto da janela quando a descoberta ainda não viu nada).
    let mut node_lines: Vec<(String, bool, egui::Pos2, egui::Pos2)> = Vec::new();
    let nodes: Vec<(String, bool, u64)> = if discovery.nodes.is_empty() {
        dds.snapshot
            .studio_nodes
            .iter()
            .map(|row| (row.node_id.clone(), true, 0))
            .collect()
    } else {
        discovery
            .nodes
            .iter()
            .map(|node| (node.node_id.clone(), node.is_alive(now), node.age_secs(now)))
            .collect()
    };
    let count = nodes.len().min(6);
    if count > 0 {
        let radius = (rect.width() * 0.30).clamp(120.0, 260.0);
        for (index, (node_id, alive, age)) in nodes.iter().take(count).enumerate() {
            // Arco superior: -160°…-20° (esq.→dir.).
            let angle = (-160.0 + (140.0 / (count as f32 - 1.0).max(1.0)) * index as f32)
                .to_radians();
            let pos = center + egui::vec2(angle.cos() * radius, angle.sin() * radius);
            let box_rect = egui::Rect::from_center_size(pos, egui::vec2(132.0, 34.0));
            let accent = if *alive { theme::OK } else { theme::STALE };
            painter.line_segment(
                [center + egui::vec2(0.0, -24.0), box_rect.center()],
                egui::Stroke::new(1.0, theme::tint(accent, 60)),
            );
            painter.rect_filled(
                box_rect,
                egui::CornerRadius::same(3),
                theme::SURFACE_CONTAINER,
            );
            painter.rect_stroke(
                box_rect,
                egui::CornerRadius::same(3),
                egui::Stroke::new(1.0, theme::SURFACE_HIGHEST),
                egui::StrokeKind::Inside,
            );
            painter.text(
                egui::pos2(box_rect.left() + 6.0, box_rect.top() + 5.0),
                egui::Align2::LEFT_TOP,
                node_id,
                egui::FontId::monospace(9.0),
                theme::ON_SURFACE,
            );
            painter.text(
                egui::pos2(box_rect.left() + 6.0, box_rect.top() + 19.0),
                egui::Align2::LEFT_TOP,
                format!("hb {age}s {}", if *alive { "● vivo" } else { "◌ expirado" }),
                egui::FontId::monospace(8.0),
                accent,
            );
            node_lines.push((node_id.clone(), *alive, center, box_rect.center()));
        }
        // Rótulo do tópico na primeira linha (sem poluir as demais).
        if let Some((_, _, a, b)) = node_lines.first() {
            let mid = (*a + b.to_vec2()) / 2.0;
            painter.text(
                mid,
                egui::Align2::CENTER_CENTER,
                "Studio.NodePresence",
                egui::FontId::monospace(8.0),
                theme::OUTLINE,
            );
        }
    } else {
        painter.text(
            egui::pos2(center.x, rect.top() + 60.0),
            egui::Align2::CENTER_CENTER,
            "nenhum nó visto — aguardando heartbeats em Studio.NodePresence…",
            egui::FontId::monospace(9.0),
            theme::STALE,
        );
    }

    // Enxame IA (AgentRegistry) na lateral direita.
    let agents: Vec<&crate::discovery::AgentRow> = discovery.agents.iter().take(8).collect();
    if !agents.is_empty() {
        let column_left = rect.right() - 156.0;
        let mut top = rect.top() + 56.0;
        painter.text(
            egui::pos2(column_left, top),
            egui::Align2::LEFT_TOP,
            format!("ENXAME IA · {} agente(s)", discovery.agents.len()),
            egui::FontId::monospace(9.0),
            theme::OUTLINE,
        );
        top += 16.0;
        let anchor = egui::pos2(column_left - 8.0, top + (agents.len() as f32) * 18.0 / 2.0);
        painter.line_segment(
            [center, anchor],
            egui::Stroke::new(1.0, theme::tint(theme::PRIMARY_FIXED_DIM, 60)),
        );
        painter.text(
            egui::pos2((center.x + anchor.x) / 2.0, center.y - 10.0),
            egui::Align2::CENTER_CENTER,
            "AgentRegistry",
            egui::FontId::monospace(8.0),
            theme::OUTLINE,
        );
        for agent in agents {
            let box_rect = egui::Rect::from_min_size(
                egui::pos2(column_left, top),
                egui::vec2(148.0, 16.0),
            );
            painter.rect_filled(
                box_rect,
                egui::CornerRadius::same(2),
                theme::SURFACE_CONTAINER,
            );
            painter.text(
                egui::pos2(box_rect.left() + 4.0, box_rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!(
                    "{} · {}/{} slots",
                    agent.agent_id, agent.slots_busy, agent.slots_total
                ),
                egui::FontId::monospace(8.0),
                theme::ON_SURFACE_VARIANT,
            );
            top += 18.0;
        }
    }

    // Servidor de inferência (ServerStatus) na lateral esquerda.
    if let Some(server) = discovery.servers.first() {
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 16.0, rect.bottom() - 64.0),
            egui::vec2(168.0, 40.0),
        );
        painter.line_segment(
            [center, box_rect.center()],
            egui::Stroke::new(1.0, theme::tint(theme::WARN, 60)),
        );
        painter.text(
            egui::pos2((center.x + box_rect.center().x) / 2.0, center.y + 16.0),
            egui::Align2::CENTER_CENTER,
            "ServerStatus",
            egui::FontId::monospace(8.0),
            theme::OUTLINE,
        );
        painter.rect_filled(
            box_rect,
            egui::CornerRadius::same(3),
            theme::SURFACE_CONTAINER,
        );
        painter.rect_stroke(
            box_rect,
            egui::CornerRadius::same(3),
            egui::Stroke::new(1.0, theme::SURFACE_HIGHEST),
            egui::StrokeKind::Inside,
        );
        painter.text(
            egui::pos2(box_rect.left() + 6.0, box_rect.top() + 5.0),
            egui::Align2::LEFT_TOP,
            &server.server_id,
            egui::FontId::monospace(9.0),
            theme::ON_SURFACE,
        );
        painter.text(
            egui::pos2(box_rect.left() + 6.0, box_rect.top() + 20.0),
            egui::Align2::LEFT_TOP,
            format!(
                "⚡ {} · slots {}/{} · pronto={}",
                server.model_loaded,
                server.slots_processing,
                server.slots_idle + server.slots_processing,
                if server.ready { "sim" } else { "não" }
            ),
            egui::FontId::monospace(8.0),
            theme::ON_SURFACE_VARIANT,
        );
    }
}

// ── Abas de coleções ─────────────────────────────────────────────────────

/// Aba Agentes DDS (fonte contínua: descoberta/AgentRegistry).
fn agents_tab(ui: &mut egui::Ui, discovery: &DiscoveryState) {
    if discovery.agents.is_empty() {
        kit::empty_state(
            ui,
            "Nenhum agente com heartbeat no domínio (AgentRegistry, poda 30 s) — \
             abra um agente no domínio e ele aparece sozinho aqui.",
        );
        return;
    }
    let now = now_unix_ns();
    egui::Grid::new("topology_agents_grid")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(ui, &["Identificação", "Modelo", "Slots", "Latência EMA", "Heartbeat"]);
            for agent in &discovery.agents {
                kit::mono_cell(ui, &agent.agent_id);
                ui.label(&agent.model);
                ui.label(format!("{}/{}", agent.slots_busy, agent.slots_total));
                ui.label(format!("{:.0} ms", agent.ema_latency_ms));
                let age = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
                ui.label(
                    egui::RichText::new(format!("{age} s atrás"))
                        .color(if age <= 5 { theme::OK } else { theme::WARN })
                        .monospace(),
                );
                ui.end_row();
            }
        });
}

/// Aba Tool Calls (janela de observação; governança requester/nível).
fn tools_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.tools.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma tool call na janela — gere uma chamada (agente com engine LLM \
             + mcp-gateway + policy-engine no domínio).",
        );
        return;
    }
    egui::Grid::new("topology_tools_grid")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(
                ui,
                &["Call ID", "Ferramenta", "Solicitante", "Nível", "Status", "Resultado (prévia)"],
            );
            for tool in &snapshot.tools {
                let status = crate::dds_observe::status_label(tool.status);
                if !filter.is_empty()
                    && !matches(&[
                        &tool.call_id,
                        &tool.tool_name,
                        &tool.requester_id,
                        status,
                        &tool.result_preview,
                    ])
                {
                    continue;
                }
                let short_id: String = tool.call_id.chars().take(8).collect();
                kit::mono_cell(ui, &short_id);
                ui.label(&tool.tool_name);
                ui.label(&tool.requester_id);
                kit::mono_cell(ui, &crate::dds_observe::security_level_label(tool.security_level));
                let status_color = match tool.status {
                    2 | 5 => theme::ERROR,
                    4 => theme::OK,
                    _ => theme::ON_SURFACE_VARIANT,
                };
                ui.label(
                    egui::RichText::new(status).monospace().color(status_color),
                );
                ui.label(
                    egui::RichText::new(&tool.result_preview)
                        .small()
                        .weak(),
                );
                ui.end_row();
            }
        });
}

/// Aba Métricas (SystemMetric drenadas na janela).
fn metrics_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.metrics.is_empty() {
        kit::empty_state(ui, "Nenhuma métrica de sistema na janela.");
        return;
    }
    egui::Grid::new("topology_metrics_grid")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(ui, &["Origem", "Métrica (unidade)", "Valor"]);
            for metric in &snapshot.metrics {
                if !filter.is_empty()
                    && !matches(&[&metric.source, &metric.name])
                {
                    continue;
                }
                kit::mono_cell(ui, &metric.source);
                ui.label(&metric.name);
                ui.label(
                    egui::RichText::new(format!("{:.3}", metric.value)).monospace(),
                );
                ui.end_row();
            }
        });
}

/// Aba Descoberta DDS (eventos drenados na janela).
fn discovery_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.discoveries.is_empty() {
        kit::empty_state(ui, "Nenhum evento de descoberta na janela.");
        return;
    }
    egui::Grid::new("topology_discovery_grid")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(ui, &["Evento", "Tópico", "Entidade remota"]);
            for event in &snapshot.discoveries {
                if !filter.is_empty()
                    && !matches(&[&event.event_type, &event.topic_name, &event.remote_entity])
                {
                    continue;
                }
                ui.label(&event.event_type);
                kit::mono_cell(ui, &event.topic_name);
                kit::mono_cell(ui, &event.remote_entity);
                ui.end_row();
            }
        });
}

/// Aba Instalações (presença contínua: nós Studio com probe e idade).
fn installations_tab(ui: &mut egui::Ui, discovery: &DiscoveryState) {
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma instalação vista no domínio — nós com studio-node publicam \
             presença a cada 5 s.",
        );
        return;
    }
    let now = now_unix_ns();
    egui::Grid::new("topology_nodes_grid")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(
                ui,
                &["Node ID", "URL", "Estado do probe", "Idade HB", "Token exigido"],
            );
            for node in &discovery.nodes {
                kit::mono_cell(ui, &node.node_id);
                kit::mono_cell(ui, &node.url);
                let (color, detail) = match &node.probe {
                    Some(probe) => match probe.state {
                        ProbeState::Online => (theme::OK, probe.detail.clone()),
                        ProbeState::AuthPending => (theme::AUTH, probe.detail.clone()),
                        ProbeState::Offline | ProbeState::Unknown => {
                            (theme::ERROR, probe.detail.clone())
                        }
                    },
                    None => (theme::STALE, String::from("sondando…")),
                };
                ui.label(
                    egui::RichText::new(format!("● {detail}"))
                        .monospace()
                        .small()
                        .color(color),
                );
                ui.label(format!("{} s", node.age_secs(now)));
                ui.label(if node.token_required { "sim" } else { "não" });
                ui.end_row();
            }
        });
}
