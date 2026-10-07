//! Painel 3.11 Topologia DDS & Enxame Distribuído (tela inicial do Studio).
//!
//! Refeito sobre o mockup Stitch atual + PRD v1.0: kicker com QoS do
//! contrato, janela em chips, faixa de 6 contadores, chips de filtro por
//! tópico canônico, mesh desenhado com `Painter` (cards clicáveis →
//! "ASSINANTE SELECIONADO") e painel de 7 abas — a primeira é TAREFAS DDS
//! (Tasks ao vivo) com barra de log de `TaskOutput` no rodapé.
//! Só dados reais: nada de GUID/RTT/throughput inventado.

use crate::dds_observe::{task_status_label, DdsSnapshot, DdsState, TaskRow};
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::{now_unix_ns, ProbeState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Presets de janela do mockup (chips 1/5/10/15/30 s).
const WINDOW_PRESETS: [u64; 5] = [1, 5, 10, 15, 30];

/// Abas do painel de coleções (índice = `DdsState::tab`).
const TABS: [&str; 7] = [
    "Tarefas DDS",
    "Agentes DDS",
    "Server Status",
    "Tool Calls",
    "Métricas",
    "Descoberta DDS",
    "Instalações",
];

/// Tópicos canônicos filtráveis (PRD 3.11) → aba correspondente.
const TOPIC_TABS: [(&str, u8); 6] = [
    ("Tasks", 0),
    ("TaskOutput", 0),
    ("AgentRegistry", 1),
    ("ServerStatus", 2),
    ("ToolCall.Request", 3),
    ("Studio.NodePresence", 6),
];

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    // Drena o worker de observação (thread + mpsc — REQ/T-820-19).
    dds.poll();
    panel_header(
        ui,
        &format!("SEC 3.11 · TOPOLOGIA DE REDE DDS · DOMÍNIO {}", dds.domain),
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
            if kit::pill_chip(ui, active, &format!("{secs} s"), theme::PRIMARY_FIXED_DIM).clicked()
            {
                dds.window_secs = secs;
            }
        }
        ui.add_enabled_ui(!dds.busy, |ui| {
            let observe = kit::primary_button(
                ui,
                if dds.busy {
                    "Observando domínio…"
                } else {
                    "Observar Domínio"
                },
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
    // (valores extraídos em escopo — o borrow do snapshot termina antes
    // do `draw_mesh`, que recebe `&mut DdsState` para a seleção do mesh.)
    let (drained, task_count, tool_count, output_count, (pending, running, done)) = {
        let snapshot = &dds.snapshot;
        let drained = snapshot.tasks.len()
            + snapshot.task_outputs.len()
            + snapshot.agents.len()
            + snapshot.tools.len()
            + snapshot.metrics.len()
            + snapshot.discoveries.len()
            + snapshot.studio_nodes.len();
        (
            drained,
            snapshot.tasks.len(),
            snapshot.tools.len(),
            snapshot.task_outputs.len(),
            task_breakdown(&snapshot.tasks),
        )
    };
    let node_ids: Vec<&str> = discovery.nodes.iter().map(|n| n.node_id.as_str()).collect();
    let agent_ids: Vec<&str> = discovery
        .agents
        .iter()
        .map(|a| a.agent_id.as_str())
        .collect();
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
            "Tarefas DDS",
            task_count.to_string(),
            &format!("{pending} pendente(s) · {running} running · {done} done"),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[5],
            "Tool Calls",
            tool_count.to_string(),
            &format!("janela de {} s", dds.window_secs),
            theme::PRIMARY_FIXED_DIM,
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Mesh (Painter): estação local no centro + nós + enxame + inferência ──
    draw_mesh(ui, dds, discovery);
    selected_subscriber_card(ui, dds, discovery);
    ui.label(
        egui::RichText::new(
            "TÓPICOS FILTRÁVEIS: Tasks · TaskOutput · AgentRegistry · ServerStatus · \
             ToolCall.Request · Studio.NodePresence",
        )
        .monospace()
        .small()
        .color(theme::OUTLINE),
    );
    ui.add_space(theme::SPACE_MD);

    // ── Chips de filtro por tópico canônico (PRD 3.11) ──
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("FILTRO:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        let counts = [
            task_count,              // Tasks
            output_count,            // TaskOutput
            discovery.agents.len(),  // AgentRegistry
            discovery.servers.len(), // ServerStatus
            tool_count,              // ToolCall.Request
            discovery.nodes.len(),   // Studio.NodePresence
        ];
        for (index, (topic, tab)) in TOPIC_TABS.iter().enumerate() {
            let active = dds.tab == *tab && topic_active_for_tab(topic, dds.tab);
            let chip = format!("{topic} ({})", counts[index]);
            if kit::pill_chip(ui, active, &chip, theme::PRIMARY_FIXED_DIM).clicked() {
                dds.tab = *tab;
            }
        }
    });
    ui.add_space(theme::SPACE_XS);

    // ── Painel de 7 abas com filtro (UX3: pills) ──
    ui.horizontal(|ui| {
        for (index, name) in TABS.iter().enumerate() {
            let active = dds.tab as usize == index;
            if kit::pill_chip(ui, active, name, theme::PRIMARY_FIXED_DIM).clicked() {
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
    let tab = dds.tab;
    let snapshot = &dds.snapshot;
    match tab {
        0 => tasks_tab(ui, snapshot, &mut dds.task_selected, &filter, matches),
        1 => agents_tab(ui, discovery),
        2 => servers_tab(ui, discovery),
        3 => tools_tab(ui, snapshot, &filter, matches),
        4 => metrics_tab(ui, snapshot, &filter, matches),
        5 => discovery_tab(ui, snapshot, &filter, matches),
        _ => installations_tab(ui, discovery),
    }

    // ── Barra de log de tarefas (TaskOutput ao vivo — rodapé da 3.11) ──
    task_log_bar(ui, snapshot, &mut dds.task_selected);
}

/// Chip "Tasks"/"TaskOutput" só fica ativo na aba 0 (ambos apontam lá).
fn topic_active_for_tab(topic: &str, tab: u8) -> bool {
    if tab == 0 {
        topic != "TaskOutput"
    } else {
        true
    }
}

/// Contagem por estado do ciclo canônico (pendente = PENDING+ASSIGNED).
fn task_breakdown(tasks: &[TaskRow]) -> (usize, usize, usize) {
    let pending = tasks
        .iter()
        .filter(|t| t.status == 0 || t.status == 1)
        .count();
    let running = tasks.iter().filter(|t| t.status == 2).count();
    let done = tasks.iter().filter(|t| t.status == 3).count();
    (pending, running, done)
}

/// `true` quando alguma coleção da janela tem linha (para o selo CONVERGIDA).
fn snapshot_has_data(snapshot: &DdsSnapshot) -> bool {
    !(snapshot.tasks.is_empty()
        && snapshot.task_outputs.is_empty()
        && snapshot.agents.is_empty()
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

/// Prévia de 96 chars achatada (padrão PRD para conteúdos de tarefa).
fn preview96(text: &str) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    if flat.chars().count() <= 96 {
        flat
    } else {
        format!("{}…", flat.chars().take(96).collect::<String>())
    }
}

/// Cor do estado do ciclo de vida da tarefa.
fn task_status_color(status: i32) -> egui::Color32 {
    match status {
        0 | 1 => theme::STALE,
        2 => theme::PRIMARY_FIXED_DIM,
        3 => theme::OK,
        4 => theme::ERROR,
        _ => theme::ON_SURFACE_VARIANT,
    }
}

// ── Mesh ─────────────────────────────────────────────────────────────────

/// Mesh (UX4): canvas DOMINANTE com CARDS DE ASSINANTE GRANDES e ricos
/// (título + 4-5 linhas de propriedades reais + faixa colorida por tipo),
/// espalhados ao redor do hub ESTAÇÃO LOCAL e conectados por linhas — a
/// composição do design Stitch (não um diagrama radial minúsculo). Cards
/// clicáveis: a seleção vira o card "ASSINANTE SELECIONADO".
fn draw_mesh(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    let height = 420.0;
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
    painter.text(
        egui::pos2(rect.left() + theme::SPACE_LG, rect.top() + theme::SPACE_MD),
        egui::Align2::LEFT_TOP,
        "TOPOLOGIA DE BARRAMENTO DDS · MODO OBSERVADOR",
        egui::FontId::monospace(10.0),
        theme::OUTLINE,
    );

    let now = now_unix_ns();
    let w = rect.width();
    let h = rect.height();
    let pt = |fx: f32, fy: f32| egui::pos2(rect.left() + w * fx, rect.top() + h * fy);

    // Cards clicáveis coletados para o hit-test do clique.
    let mut hit_rects: Vec<(egui::Rect, String)> = Vec::new();

    // ── Hub: ESTAÇÃO LOCAL (este Studio) ──
    let hub_rect = egui::Rect::from_center_size(pt(0.50, 0.50), egui::vec2(190.0, 58.0));
    painter.rect_filled(hub_rect, egui::CornerRadius::same(6), theme::SURFACE_HIGH);
    painter.rect_stroke(
        hub_rect,
        egui::CornerRadius::same(6),
        egui::Stroke::new(1.25, theme::PRIMARY_CONTAINER),
        egui::StrokeKind::Inside,
    );
    painter.text(
        hub_rect.left_top() + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        "ESTAÇÃO LOCAL",
        egui::FontId::monospace(11.0),
        theme::PRIMARY_FIXED_DIM,
    );
    painter.text(
        hub_rect.left_top() + egui::vec2(10.0, 26.0),
        egui::Align2::LEFT_TOP,
        "STUDIO GUI · observador",
        egui::FontId::monospace(9.0),
        theme::ON_SURFACE_VARIANT,
    );

    // Desenha um card de assinante GRANDE (UX4): faixa superior colorida por
    // tipo, título + dot, e 3-4 linhas de propriedades reais.
    #[allow(clippy::too_many_arguments)]
    let subscriber_card = |painter: &egui::Painter,
                           center: egui::Pos2,
                           title: &str,
                           lines: &[String],
                           accent: egui::Color32,
                           alive: bool,
                           selected: bool,
                           hit: &mut Vec<(egui::Rect, String)>,
                           key: String| {
        let size = egui::vec2(258.0, 30.0 + lines.len() as f32 * 15.0);
        let card = egui::Rect::from_center_size(center, size);
        // Linha do hub ao card.
        painter.line_segment(
            [hub_rect.center(), card.center()],
            egui::Stroke::new(1.5, theme::tint(accent, 80)),
        );
        painter.rect_filled(card, egui::CornerRadius::same(6), theme::SURFACE_CONTAINER);
        painter.rect_stroke(
            card,
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
        let strip = card.with_max_y(card.top() + 3.0);
        painter.rect_filled(
            strip,
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
            egui::pos2(card.left() + 12.0, card.top() + 17.0),
            3.5,
            if alive { accent } else { theme::STALE },
        );
        painter.text(
            egui::pos2(card.left() + 21.0, card.top() + 9.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::monospace(10.5),
            theme::ON_SURFACE,
        );
        for (index, line) in lines.iter().enumerate() {
            painter.text(
                egui::pos2(card.left() + 12.0, card.top() + 28.0 + index as f32 * 15.0),
                egui::Align2::LEFT_TOP,
                line,
                egui::FontId::monospace(8.75),
                theme::ON_SURFACE_VARIANT,
            );
        }
        hit.push((card, key));
    };

    // ── Nós Studio (Studio.NodePresence) — colunas à direita/esquerda ──
    let nodes: Vec<(String, bool, u64, String)> = if discovery.nodes.is_empty() {
        dds.snapshot
            .studio_nodes
            .iter()
            .map(|row| (row.node_id.clone(), true, 0, row.url.clone()))
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
                )
            })
            .collect()
    };
    let node_slots = [
        (0.78, 0.16),
        (0.78, 0.52),
        (0.78, 0.86),
        (0.20, 0.16),
        (0.20, 0.86),
    ];
    for (index, (node_id, alive, age, url)) in nodes.iter().take(5).enumerate() {
        let (fx, fy) = node_slots[index];
        let accent = if *alive { theme::OK } else { theme::STALE };
        let selected = dds.mesh_selected.as_deref() == Some(node_id.as_str());
        subscriber_card(
            &painter,
            pt(fx, fy),
            &format!("NÓ · {node_id}"),
            &[
                format!("url {url}"),
                format!("heartbeat {age}s atrás · lease 10s"),
                String::from("QoS Reliable + TransientLocal"),
                format!(
                    "publica Studio.NodePresence (5s) · token {}",
                    if discovery
                        .nodes
                        .iter()
                        .any(|n| n.node_id == *node_id && n.token_required)
                    {
                        "exigido"
                    } else {
                        "livre"
                    }
                ),
            ],
            accent,
            *alive,
            selected,
            &mut hit_rects,
            format!("node:{node_id}"),
        );
    }

    // ── Enxame IA (AgentRegistry) — coluna esquerda ──
    let agent_slots = [(0.20, 0.34), (0.20, 0.60)];
    for (index, agent) in discovery.agents.iter().take(2).enumerate() {
        let (fx, fy) = agent_slots[index];
        let age = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
        let selected = dds.mesh_selected.as_deref() == Some(agent.agent_id.as_str());
        subscriber_card(
            &painter,
            pt(fx, fy),
            &format!("AGENTE · {}", agent.agent_id),
            &[
                format!("modelo {}", agent.model),
                format!(
                    "slots {}/{} · latência EMA {:.0} ms",
                    agent.slots_busy, agent.slots_total, agent.ema_latency_ms
                ),
                format!("heartbeat {age}s atrás"),
                String::from("subscreve Tasks · publica TaskOutput"),
            ],
            theme::PRIMARY_FIXED_DIM,
            age <= 30,
            selected,
            &mut hit_rects,
            format!("agent:{}", agent.agent_id),
        );
    }

    // ── Servidor de inferência (ServerStatus) — topo centro ──
    if let Some(server) = discovery.servers.first() {
        let selected = dds.mesh_selected.as_deref() == Some(server.server_id.as_str());
        subscriber_card(
            &painter,
            pt(0.50, 0.16),
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
            selected,
            &mut hit_rects,
            format!("server:{}", server.server_id),
        );
    }

    // Rótulo do tópico das linhas do hub.
    painter.text(
        pt(0.50, 0.565),
        egui::Align2::CENTER_CENTER,
        "Studio.NodePresence · AgentRegistry · ServerStatus",
        egui::FontId::monospace(8.5),
        theme::OUTLINE,
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

/// Card de detalhe do assinante selecionado no mesh (mockup: "SELECTED
/// SUBSCRIBER") — dados reais do tipo correspondente + tópicos que publica.
fn selected_subscriber_card(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    let Some(key) = dds.mesh_selected.clone() else {
        return;
    };
    kit::accent_card(ui, theme::PRIMARY_CONTAINER, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("ASSINANTE SELECIONADO")
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("limpar seleção").monospace().small(),
                ))
                .clicked()
            {
                dds.mesh_selected = None;
            }
        });
        let now = now_unix_ns();
        if let Some(node_id) = key.strip_prefix("node:") {
            if let Some(node) = discovery.nodes.iter().find(|n| n.node_id == node_id) {
                kit::mono_cell(ui, &format!("NÓ STUDIO · {}", node.node_id));
                ui.label(format!(
                    "url {} · token exigido: {}",
                    node.url,
                    if node.token_required { "sim" } else { "não" }
                ));
                let (probe, color) = match &node.probe {
                    Some(probe) => match probe.state {
                        ProbeState::Online => (probe.detail.clone(), theme::OK),
                        ProbeState::AuthPending => (probe.detail.clone(), theme::AUTH),
                        ProbeState::Offline | ProbeState::Unknown => {
                            (probe.detail.clone(), theme::ERROR)
                        }
                    },
                    None => (String::from("sondando…"), theme::STALE),
                };
                ui.label(
                    egui::RichText::new(format!(
                        "probe {probe} · heartbeat {} s atrás",
                        node.age_secs(now)
                    ))
                    .small()
                    .color(color),
                );
                ui.label(
                    egui::RichText::new("publica: Studio.NodePresence (heartbeat 5 s)")
                        .small()
                        .weak(),
                );
                return;
            }
        }
        if let Some(agent_id) = key.strip_prefix("agent:") {
            if let Some(agent) = discovery.agents.iter().find(|a| a.agent_id == agent_id) {
                kit::mono_cell(ui, &format!("AGENTE DDS · {}", agent.agent_id));
                ui.label(format!(
                    "modelo {} · slots {}/{} · latência EMA {:.0} ms",
                    agent.model, agent.slots_busy, agent.slots_total, agent.ema_latency_ms
                ));
                let age = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
                ui.label(
                    egui::RichText::new(format!("heartbeat {age} s atrás"))
                        .small()
                        .color(if age <= 5 { theme::OK } else { theme::WARN }),
                );
                ui.label(
                    egui::RichText::new(
                        "publica: AgentRegistry · subscreve: Tasks (claim) · publica: TaskOutput",
                    )
                    .small()
                    .weak(),
                );
                return;
            }
        }
        if let Some(server_id) = key.strip_prefix("server:") {
            if let Some(server) = discovery.servers.iter().find(|s| s.server_id == server_id) {
                kit::mono_cell(
                    ui,
                    &format!("SERVIDOR DE INFERÊNCIA · {}", server.server_id),
                );
                ui.label(format!(
                    "modelo {} · slots {}/{} · pronto: {}",
                    server.model_loaded,
                    server.slots_processing,
                    server.slots_idle + server.slots_processing,
                    if server.ready { "sim" } else { "não" }
                ));
                ui.label(egui::RichText::new("publica: ServerStatus").small().weak());
                return;
            }
        }
        ui.label(egui::RichText::new("seleção saiu do domínio").weak());
    });
}

// ── Abas de coleções ─────────────────────────────────────────────────────

/// Aba Tarefas DDS (Tasks ao vivo — ciclo, agente, prioridade, retry).
fn tasks_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    selected: &mut Option<String>,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.tasks.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma tarefa no tópico Tasks na janela — despache pela tela 3.6 ou \
             via orquestrador/agente; o ciclo PENDING→ASSIGNED→RUNNING→DONE aparece aqui.",
        );
        return;
    }
    let now = now_unix_ns();
    kit::table("topology_tasks_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Task ID",
                "Estado",
                "Agente",
                "Modelo",
                "Prioridade",
                "Retry",
                "Idade",
            ],
        );
        for task in &snapshot.tasks {
            let status = task_status_label(task.status);
            if !filter.is_empty()
                && !matches(&[
                    &task.task_id,
                    status,
                    &task.assigned_agent,
                    &task.model_name,
                ])
            {
                continue;
            }
            let is_selected = selected.as_deref() == Some(task.task_id.as_str());
            let short_id: String = task.task_id.chars().take(8).collect();
            if ui
                .selectable_label(is_selected, egui::RichText::new(&short_id).monospace())
                .clicked()
            {
                *selected = Some(task.task_id.clone());
            }
            ui.label(
                egui::RichText::new(status)
                    .monospace()
                    .color(task_status_color(task.status)),
            );
            ui.label(if task.assigned_agent.is_empty() {
                "—"
            } else {
                &task.assigned_agent
            });
            ui.label(if task.model_name.is_empty() {
                "—"
            } else {
                &task.model_name
            });
            kit::num_cell(ui, &task.priority.to_string());
            kit::num_cell(ui, &task.retry_count.to_string());
            let age = now.saturating_sub(task.created_at_ns) / 1_000_000_000;
            kit::num_cell(ui, &format!("{age} s"));
            ui.end_row();
        }
    });
}

/// Barra de log de tarefas: `TaskOutput` ao vivo (mais recentes primeiro),
/// com prévia de 96 chars; seleção na aba Tarefas filtra este log.
fn task_log_bar(ui: &mut egui::Ui, snapshot: &DdsSnapshot, selected: &mut Option<String>) {
    if snapshot.task_outputs.is_empty() {
        return;
    }
    ui.add_space(theme::SPACE_MD);
    kit::section_label(ui, "Log de tarefas · tópico TaskOutput (janela atual)");
    if let Some(task_id) = selected.clone() {
        ui.horizontal(|ui| {
            let short: String = task_id.chars().take(8).collect();
            ui.label(
                egui::RichText::new(format!("FILTRADO: {short}"))
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("limpar filtro").monospace().small(),
                ))
                .clicked()
            {
                *selected = None;
            }
        });
    }
    let rows: Vec<&crate::dds_observe::TaskOutputRow> = snapshot
        .task_outputs
        .iter()
        .filter(|row| selected.as_ref().is_none_or(|id| row.task_id == *id))
        .collect();
    kit::table("topology_tasklog_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Task ID",
                "Seq",
                "Agente",
                "Final",
                "Tokens",
                "Conteúdo (96 chars)",
            ],
        );
        for row in rows.iter().rev().take(12) {
            let short: String = row.task_id.chars().take(8).collect();
            kit::mono_cell(ui, &short);
            kit::num_cell(ui, &row.seq_num.to_string());
            ui.label(&row.agent_id);
            ui.label(if row.is_final { "✔" } else { "…" });
            kit::num_cell(ui, &row.token_count.to_string());
            kit::mono_cell(ui, &preview96(&row.content));
            ui.end_row();
        }
    });
    if rows.len() > 12 {
        ui.label(
            egui::RichText::new(format!(
                "+{} amostra(s) mais antigas na janela",
                rows.len() - 12
            ))
            .small()
            .weak(),
        );
    }
}

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
    kit::table("topology_agents_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Identificação",
                "Modelo",
                "Slots",
                "Latência EMA",
                "Heartbeat",
            ],
        );
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

/// Aba Server Status (inferência viva — descoberta contínua).
fn servers_tab(ui: &mut egui::Ui, discovery: &DiscoveryState) {
    if discovery.servers.is_empty() {
        kit::empty_state(
            ui,
            "Nenhum servidor anunciando ServerStatus no domínio — o llama-server \
             precisa ser iniciado com --enable-dds para publicar presença.",
        );
        return;
    }
    kit::table("topology_servers_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Server ID",
                "Modelo carregado",
                "Slots idle",
                "Slots proc.",
                "Pronto",
            ],
        );
        for server in &discovery.servers {
            kit::mono_cell(ui, &server.server_id);
            ui.label(&server.model_loaded);
            kit::num_cell(ui, &server.slots_idle.to_string());
            kit::num_cell(ui, &server.slots_processing.to_string());
            ui.label(
                egui::RichText::new(if server.ready { "● sim" } else { "◌ não" })
                    .monospace()
                    .color(if server.ready { theme::OK } else { theme::WARN }),
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
    kit::table("topology_tools_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Call ID",
                "Ferramenta",
                "Solicitante",
                "Nível",
                "Status",
                "Duração",
                "Resultado (prévia)",
            ],
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
            kit::mono_cell(
                ui,
                &crate::dds_observe::security_level_label(tool.security_level),
            );
            let status_color = match tool.status {
                2 | 5 => theme::ERROR,
                4 => theme::OK,
                _ => theme::ON_SURFACE_VARIANT,
            };
            ui.label(egui::RichText::new(status).monospace().color(status_color));
            kit::num_cell(
                ui,
                &if tool.duration_ms > 0 {
                    format!("{} ms", tool.duration_ms)
                } else {
                    String::from("aberta")
                },
            );
            ui.label(egui::RichText::new(&tool.result_preview).small().weak());
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
    kit::table("topology_metrics_grid").show(ui, |ui| {
        kit::grid_header(ui, &["Origem", "Métrica (unidade)", "Valor"]);
        for metric in &snapshot.metrics {
            if !filter.is_empty() && !matches(&[&metric.source, &metric.name]) {
                continue;
            }
            kit::mono_cell(ui, &metric.source);
            ui.label(&metric.name);
            kit::num_cell(ui, &format!("{:.3}", metric.value));
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
    kit::table("topology_discovery_grid").show(ui, |ui| {
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
    kit::table("topology_nodes_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Node ID",
                "URL",
                "Estado do probe",
                "Idade HB",
                "Token exigido",
            ],
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
            kit::num_cell(ui, &format!("{} s", node.age_secs(now)));
            ui.label(if node.token_required { "sim" } else { "não" });
            ui.end_row();
        }
    });
}
