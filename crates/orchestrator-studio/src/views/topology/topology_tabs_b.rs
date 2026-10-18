//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

/// Aba Agentes DDS (fonte contínua: descoberta/AgentRegistry).
pub(super) fn agents_tab(ui: &mut egui::Ui, discovery: &DiscoveryState) {
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
pub(super) fn servers_tab(ui: &mut egui::Ui, discovery: &DiscoveryState) {
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
pub(super) fn tools_tab(
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
