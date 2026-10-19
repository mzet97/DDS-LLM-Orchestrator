//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

/// Card de detalhe do assinante selecionado no mesh (mockup: "SELECTED
/// SUBSCRIBER") — dados reais do tipo correspondente + tópicos que publica.
pub(super) fn selected_subscriber_card(
    ui: &mut egui::Ui,
    dds: &mut DdsState,
    discovery: &DiscoveryState,
) {
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
