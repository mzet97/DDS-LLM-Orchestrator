//! Parte de `views::agents` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_registry(
    ui: &mut egui::Ui,
    _agents: &mut AgentsState,
    discovery: &DiscoveryState,
) {
    // ── Seção A: AgentRegistry ao vivo (mockup) ──
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Seção A: Agentes Ativos no Domínio DDS")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            kit::badge(ui, "Tópico: AgentRegistry", theme::OUTLINE);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Heartbeat: 1000ms")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new("Lease: 30s poda automática")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "● {} Agentes Registrados no DDS",
                        discovery.agents.len()
                    ))
                    .monospace()
                    .small()
                    .strong()
                    .color(theme::PRIMARY_FIXED_DIM),
                );
            });
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
            kit::table("dds_agents_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "IDENTIFICAÇÃO",
                        "MODELO CARREGADO",
                        "SLOTS ATIVOS",
                        "LATÊNCIA EMA",
                        "HEARTBEAT RTPS (LEASE 30S)",
                    ],
                );
                for agent in &discovery.agents {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("● {}", agent.agent_id))
                                .strong()
                                .color(theme::ON_SURFACE),
                        );
                    });
                    ui.label(
                        egui::RichText::new(&agent.model)
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    let frac = if agent.slots_total == 0 {
                        0.0
                    } else {
                        agent.slots_busy as f32 / agent.slots_total as f32
                    };
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}/{} em uso",
                                agent.slots_busy, agent.slots_total
                            ))
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.add(egui::ProgressBar::new(frac.clamp(0.0, 1.0)));
                    });
                    kit::num_cell(ui, &format!("{:.0} ms", agent.ema_latency_ms));
                    let age_s = now.saturating_sub(agent.last_update_ns) / 1_000_000_000;
                    let left_s = 30_u64.saturating_sub(age_s);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{age_s}s atrás · {left_s}s/30s{}",
                                if age_s <= 30 { " [VIVO]" } else { "" }
                            ))
                            .monospace()
                            .small()
                            .color(if age_s <= 5 {
                                theme::OK
                            } else {
                                theme::WARN
                            }),
                        );
                        ui.add(egui::ProgressBar::new(left_s as f32 / 30.0));
                    });
                    ui.end_row();
                }
            });
            ui.add_space(theme::SPACE_XS);
            ui.label(
                egui::RichText::new(
                    "SPDP Multicast: 239.255.0.1:7400 · Poda de lease configurada \
                     para 30000ms sem resposta.",
                )
                .monospace()
                .small()
                .color(theme::OUTLINE),
            );
        }
    });
    ui.add_space(theme::SPACE_LG);
}
