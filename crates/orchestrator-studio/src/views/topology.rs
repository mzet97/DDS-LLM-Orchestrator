//! Painel 3.11 Topologia DDS & Enxame Distribuído (tela inicial do Studio).
//!
//! Refeito sobre o mockup Stitch atual + PRD v1.0: kicker com QoS do
//! contrato, janela em chips, faixa de 6 contadores, chips de filtro por
//! tópico canônico, mesh desenhado com `Painter` (cards clicáveis →
//! "ASSINANTE SELECIONADO") e painel de 7 abas — a primeira é TAREFAS DDS
//! (Tasks ao vivo) com barra de log de `TaskOutput` no rodapé.
//! Só dados reais: nada de GUID/RTT/throughput inventado.

use crate::dds_observe::{task_status_label, DdsSnapshot, DdsState, ToolRow};
use crate::discovery::DiscoveryState;
use crate::kit::{self, short_agent, short_host};
use crate::machines::{now_unix_ns, ProbeState};
use crate::theme;
use eframe::egui;

/// Presets de janela do design (dropdown 1/5/10/15/30 s).
const WINDOW_PRESETS: [u64; 5] = [1, 5, 10, 15, 30];

/// Abas do painel de coleções (índice = `DdsState::tab`; numeradas no design).
const TABS: [&str; 7] = [
    "1. Tarefas DDS",
    "2. Agentes DDS",
    "3. Server Status",
    "4. Tool Calls",
    "5. Métricas",
    "6. Descoberta DDS",
    "7. Instalações",
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
    let drained = {
        let snapshot = &dds.snapshot;
        snapshot.tasks.len()
            + snapshot.task_outputs.len()
            + snapshot.agents.len()
            + snapshot.tools.len()
            + snapshot.metrics.len()
            + snapshot.discoveries.len()
            + snapshot.studio_nodes.len()
    };

    // ── Faixa de contexto (design: strip fino sob o topbar; rótulos reais) ──
    egui::Frame::NONE
        .fill(theme::SURFACE_LOW)
        .inner_margin(egui::Margin::symmetric(12, 5))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .small()
                        .color(theme::PRIMARY_CONTAINER),
                );
                ui.label(
                    egui::RichText::new("TOPOLOGIA DE REDE DDS")
                        .monospace()
                        .size(10.5)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "QoS: RELIABLE + TRANSIENT_LOCAL · DOMÍNIO {}",
                        dds.domain
                    ))
                    .monospace()
                    .size(10.0)
                    .color(theme::OUTLINE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "DRENADOS: {drained} MSG / {}s",
                            dds.window_secs
                        ))
                        .monospace()
                        .size(10.0)
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });
        });
    ui.separator();

    // ── Hero (design: título + ONLINE + domínio/janela/Observar à direita) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("3.11 Topologia DDS & Enxame Distribuído")
                        .size(20.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                let alive = snapshot_has_data(&dds.snapshot) || !discovery.nodes.is_empty();
                kit::badge(
                    ui,
                    if alive { "● ONLINE" } else { "○ OFFLINE" },
                    if alive { theme::OK } else { theme::STALE },
                );
            });
            ui.label(
                egui::RichText::new(
                    "Visualização estrutural de participantes de domínio, discovery \
                     leases, tópicos pub/sub e barramento RTPS.",
                )
                .small()
                .weak(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
            egui::ComboBox::from_id_salt("topology_window")
                .selected_text(format!("{} segundos", dds.window_secs))
                .show_ui(ui, |ui| {
                    for secs in WINDOW_PRESETS {
                        ui.selectable_value(&mut dds.window_secs, secs, format!("{secs} segundos"));
                    }
                });
            ui.label(
                egui::RichText::new("JANELA DDS:")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.add(egui::DragValue::new(&mut dds.domain).range(0..=230));
            ui.label(
                egui::RichText::new("DOMÍNIO:")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
    if !dds.error.is_empty() && !dds.busy {
        ui.add_space(theme::SPACE_SM);
        kit::error_banner(ui, &dds.error);
    }
    ui.add_space(theme::SPACE_MD);

    // ── Faixa de 6 contadores (design: rótulos + subtextos curtos) ──
    // (valores extraídos em escopo — o borrow do snapshot termina antes
    // do `draw_mesh`, que recebe `&mut DdsState` para a seleção do mesh.)
    let (task_count, tool_count, output_count, (tool_pending, tool_exec)) = {
        let snapshot = &dds.snapshot;
        (
            snapshot.tasks.len(),
            snapshot.tools.len(),
            snapshot.task_outputs.len(),
            tool_breakdown(&snapshot.tools),
        )
    };
    let short_nodes: Vec<String> = discovery.nodes.iter().map(|n| short_host(&n.url)).collect();
    let short_agents: Vec<String> = discovery
        .agents
        .iter()
        .map(|a| short_agent(&a.agent_id))
        .collect();
    let short_node_refs: Vec<&str> = short_nodes.iter().map(String::as_str).collect();
    let short_agent_refs: Vec<&str> = short_agents.iter().map(String::as_str).collect();
    let first_server = discovery
        .servers
        .first()
        .map(|s| s.server_id.as_str())
        .unwrap_or("—");
    let cyan = theme::PRIMARY_FIXED_DIM;
    ui.columns(6, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Total drenado",
            drained.to_string(),
            "eventos",
            cyan,
        );
        kit::metric_card(
            &mut cols[1],
            "Nós Studio",
            discovery.nodes.len().to_string(),
            &join_preview(&short_node_refs, 3),
            cyan,
        );
        kit::metric_card(
            &mut cols[2],
            "Agentes DDS",
            discovery.agents.len().to_string(),
            &short_agent_refs.join(" + "),
            cyan,
        );
        kit::metric_card(
            &mut cols[3],
            "Inference Server",
            discovery.servers.len().to_string(),
            first_server,
            cyan,
        );
        kit::metric_card(
            &mut cols[4],
            "Tool Calls",
            tool_count.to_string(),
            &format!("{tool_pending} Pend / {tool_exec} Exec"),
            cyan,
        );
        kit::metric_card(
            &mut cols[5],
            "Métricas telemetria",
            dds.snapshot.metrics.len().to_string(),
            "coletas/ciclo",
            cyan,
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Mesh colunar (design: 4 grupos lado a lado, cards clicáveis) ──
    mesh_section_header(ui);
    draw_mesh(ui, dds, discovery);
    selected_subscriber_card(ui, dds, discovery);
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
                    .hint_text("Filtrar por ID, tópico ou entidade…")
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

/// Contagem de tool calls nos grupos do design (Pend = PENDING; Exec =
/// ALLOWED + EXECUTING + COMPLETED; DENIED/FAILED ficam fora dos grupos).
fn tool_breakdown(tools: &[ToolRow]) -> (usize, usize) {
    let pending = tools.iter().filter(|t| t.status == 0).count();
    let exec = tools
        .iter()
        .filter(|t| t.status == 1 || t.status == 3 || t.status == 4)
        .count();
    (pending, exec)
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

/// Cabeçalho da seção do mesh (design: título + pill de modo + legenda).
fn mesh_section_header(ui: &mut egui::Ui) {
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
fn legend_item(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
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
fn fit_text(painter: &egui::Painter, text: &str, font: egui::FontId, max_width: f32) -> String {
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
fn mesh_card(
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

/// Mesh colunar (design 3.11): 4 grupos lado a lado — ESTAÇÃO LOCAL, NÓS
/// REMOTOS, ENXAME IA e PARTICIPANTES — com cards clicáveis (a seleção vira
/// o card "ASSINANTE SELECIONADO") e faixa de fluxos no rodapé do canvas.
/// Só dados reais: sem GUID, RTT inventado ou throughput de barramento.
fn draw_mesh(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
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
    let col_header = |painter: &egui::Painter, left: f32, width: f32, title: &str, right: &str| {
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
    };

    // ── Coluna 1: ESTAÇÃO LOCAL (este Studio, observador) ──
    col_header(&painter, col_left[0].0, col_left[0].1, "ESTAÇÃO LOCAL", "");
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

    // ── Coluna 3: ENXAME IA & DISPOSITIVOS ESPECIALIZADOS ──
    col_header(
        &painter,
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
        if key == "station" {
            kit::mono_cell(ui, "ESTAÇÃO LOCAL · STUDIO GUI (OBSERVADOR)");
            ui.label(format!(
                "domínio {} · janela {} s · {} amostras drenadas",
                dds.domain,
                dds.window_secs,
                dds.snapshot.tasks.len()
                    + dds.snapshot.task_outputs.len()
                    + dds.snapshot.agents.len()
                    + dds.snapshot.tools.len()
                    + dds.snapshot.metrics.len()
                    + dds.snapshot.discoveries.len()
                    + dds.snapshot.studio_nodes.len()
            ));
            ui.label(
                egui::RichText::new("observa: Tasks · TaskOutput · AgentRegistry · ServerStatus · ToolCall.Request · Studio.NodePresence")
                    .small()
                    .weak(),
            );
            return;
        }
        if key == "tools" {
            let (pending, exec) = tool_breakdown(&dds.snapshot.tools);
            kit::mono_cell(ui, "TOOL GATEWAY · TÓPICO ToolCall.Request");
            ui.label(format!(
                "{} chamada(s) · {pending} pendente(s) · {exec} em execução/concluída(s)",
                dds.snapshot.tools.len()
            ));
            ui.label(
                egui::RichText::new("detalhe por chamada na aba Tool Calls e na tela 3.13")
                    .small()
                    .weak(),
            );
            return;
        }
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
            ui.label(if row.is_final { "✓" } else { "…" });
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
                egui::RichText::new(if server.ready { "● sim" } else { "○ não" })
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
