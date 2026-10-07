//! Painel 3.5 Agentes: duas fontes com semântica distinta, separadas com
//! badges — FONTE 1: DDS/AgentRegistry (descoberta contínua, sempre viva) ×
//! FONTE 2: orquestrador HTTP (telemetria auxiliar; pode estar fora). Taxa
//! de sucesso e linha agregada são derivações honestas dos contadores reais.

use crate::agents::AgentsState;
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::now_unix_ns;
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Taxa de sucesso honesta: concluídas/(concluídas+falhas); `None` sem dados.
fn success_rate(completed: u64, failed: u64) -> Option<f64> {
    let total = completed + failed;
    (total > 0).then(|| completed as f64 / total as f64 * 100.0)
}

pub fn show(ui: &mut egui::Ui, agents: &mut AgentsState, discovery: &DiscoveryState) {
    panel_header(
        ui,
        &format!(
            "SEC 3.5 · ENXAME DE AGENTES · TOPOLOGIA HÍBRIDA · DOMÍNIO {}",
            discovery.domain
        ),
        "Agentes",
        "Topologia híbrida: agentes DDS em barramento peer-to-peer autônomo; \
         telemetria HTTP do orquestrador é auxiliar",
    );

    // Banner de semântica dual (mockup 3.5): quem é fonte primária.
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                kit::badge(ui, "FONTE 1: DDS RTPS (DESCOBERTA OK)", theme::OK);
                kit::badge(
                    ui,
                    if agents.error.is_empty() && !agents.list.is_empty() {
                        "FONTE 2: HTTP (SINCRONIZADO)"
                    } else {
                        "FONTE 2: HTTP (AUXILIAR — NÃO OBRIGATÓRIO)"
                    },
                    if agents.error.is_empty() {
                        theme::PRIMARY_FIXED_DIM
                    } else {
                        theme::WARN
                    },
                );
            });
            ui.label(
                egui::RichText::new(
                    "SEMÂNTICA DUAL: o barramento DDS é a fonte primária (autônoma, \
                     sem servidor central); o orquestrador HTTP (:porta) é auxiliar e \
                     NÃO introduz ponto único de falha.",
                )
                .small()
                .weak(),
            );
        });
    ui.add_space(theme::SPACE_MD);

    // ── Seção A: AgentRegistry ao vivo (descoberta contínua) ──
    kit::section_label(
        ui,
        "SEÇÃO A — AGENTES ATIVOS NO DOMÍNIO DDS · TÓPICO: AGENTREGISTRY",
    );
    ui.horizontal(|ui| {
        kit::badge(
            ui,
            &format!("{} AGENTE(S) REGISTRADO(S)", discovery.agents.len()),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::badge(ui, "LEASE: 30S PODA AUTOMÁTICA", theme::ON_SURFACE_VARIANT);
        kit::badge(ui, "HEARTBEAT: 1000MS", theme::ON_SURFACE_VARIANT);
    });
    ui.add_space(theme::SPACE_XS);
    if discovery.agents.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhum agente com heartbeat no domínio {} — agentes aparecem \
                 aqui sozinhos ao subirem.",
                discovery.domain
            ),
        );
    } else {
        let now = now_unix_ns();
        kit::table("dds_live_agents").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Identificação",
                    "Modelo",
                    "Slots",
                    "Latência EMA",
                    "Heartbeat (poda 30 s)",
                ],
            );
            for agent in &discovery.agents {
                kit::mono_cell(ui, &agent.agent_id);
                ui.label(&agent.model);
                kit::num_cell(ui, &format!("{}/{}", agent.slots_busy, agent.slots_total));
                kit::num_cell(ui, &format!("{:.0} ms", agent.ema_latency_ms));
                let age = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
                ui.label(
                    egui::RichText::new(format!(
                        "{age} s atrás · {}s/30s",
                        30_u64.saturating_sub(age)
                    ))
                    .monospace()
                    .color(if age <= 5 { theme::OK } else { theme::WARN }),
                );
                ui.end_row();
            }
        });
    }
    ui.add_space(theme::SPACE_LG);

    // ── Seção B: orquestrador HTTP (telemetria auxiliar) ──
    kit::section_label(
        ui,
        "SEÇÃO B — MÉTRICAS DE EXECUÇÃO DO ORQUESTRADOR HTTP (AUXILIAR)",
    );
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19, T-830-01).
    agents.poll();
    ui.horizontal(|ui| {
        ui.label("endpoint:");
        ui.text_edit_singleline(&mut agents.url);
        ui.add_enabled_ui(!agents.busy, |ui| {
            if ui.button("Atualizar orquestrador").clicked() {
                agents.refresh();
            }
        });
        if agents.busy {
            kit::badge(ui, "◐ LENDO…", theme::WARN);
        }
    });
    if !agents.error.is_empty() {
        // Falha honesta: o HTTP caiu, o barramento segue.
        kit::warn_banner(
            ui,
            &format!(
                "Orquestrador HTTP inacessível ({}) — o barramento DDS continua \
                 operando: a Seção A acima é a fonte primária.",
                agents.error
            ),
        );
    }
    if agents.list.is_empty() {
        kit::empty_state(ui, "Nenhum agente listado. Clique Atualizar orquestrador.");
    } else {
        egui::Grid::new("agents_grid").striped(true).show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Agente",
                    "Modelo vinculado",
                    "Host",
                    "Concluídas",
                    "Falhas",
                    "Taxa de sucesso",
                    "Latência EMA",
                ],
            );
            for agent in &agents.list {
                kit::mono_cell(ui, &agent.agent_id);
                ui.label(&agent.model);
                ui.label(&agent.hostname);
                kit::num_cell(ui, &agent.completed_total.to_string());
                kit::num_cell(ui, &agent.failed_total.to_string());
                match success_rate(agent.completed_total, agent.failed_total) {
                    Some(rate) => {
                        ui.label(
                            egui::RichText::new(format!("{rate:.2}%"))
                                .monospace()
                                .color(if rate >= 99.0 {
                                    theme::OK
                                } else if rate >= 90.0 {
                                    theme::WARN
                                } else {
                                    theme::ERROR
                                }),
                        );
                    }
                    None => {
                        ui.label("—");
                    }
                }
                kit::num_cell(ui, &format!("{:.1} ms", agent.ema_latency_ms));
                ui.end_row();
            }
            // Linha agregada do enxame (derivação honesta dos totais).
            let completed: u64 = agents.list.iter().map(|a| a.completed_total).sum();
            let failed: u64 = agents.list.iter().map(|a| a.failed_total).sum();
            let ema_avg = if agents.list.is_empty() {
                0.0
            } else {
                agents.list.iter().map(|a| a.ema_latency_ms).sum::<f32>() / agents.list.len() as f32
            };
            ui.end_row();
            ui.strong("Total agregado (enxame)");
            ui.label("");
            ui.label("");
            ui.strong(completed.to_string());
            ui.strong(failed.to_string());
            ui.strong(
                success_rate(completed, failed)
                    .map(|rate| format!("{rate:.2}%"))
                    .unwrap_or_else(|| "—".to_owned()),
            );
            ui.strong(format!("{ema_avg:.1} ms"));
            ui.end_row();
        });
    }
}
