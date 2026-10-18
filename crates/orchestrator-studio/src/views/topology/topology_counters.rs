//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_counters(
    ui: &mut egui::Ui,
    dds: &mut DdsState,
    discovery: &DiscoveryState,
    drained: usize,
) {
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
}
