//! Parte de `views::agents` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_orchestrator(
    ui: &mut egui::Ui,
    agents: &mut AgentsState,
    _discovery: &DiscoveryState,
) {
    // ── Seção B: orquestrador HTTP (mockup; telemetria auxiliar) ──
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Seção B: Métricas de Execução do Orquestrador HTTP")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Endpoint: {}/api/v1/agents",
                    agents.url.trim_end_matches('/')
                ))
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
            if agents.last_ok && agents.error.is_empty() {
                kit::badge(ui, "HTTP 200 OK", theme::OK);
            }
        });
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Último Polling: {}",
                    agents
                        .last_poll_ms
                        .map(hhmmss_ms)
                        .unwrap_or_else(|| String::from("—"))
                ))
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
            ui.add_enabled_ui(!agents.busy, |ui| {
                ui.label(
                    egui::RichText::new("URL:")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut agents.url)
                        .desired_width(220.0)
                        .hint_text("http://192.168.1.61:8080"),
                );
                if ui.button("Atualizar Orquestrador").clicked() {
                    agents.refresh();
                }
            });
            if agents.busy {
                ui.label(
                    egui::RichText::new("○ LENDO…")
                        .monospace()
                        .small()
                        .color(theme::WARN),
                );
            }
        });
        ui.add_space(theme::SPACE_XS);
        if !agents.error.is_empty() {
            kit::warn_banner(
                ui,
                &format!(
                    "Orquestrador HTTP inacessível ({}) — o barramento DDS continua \
                     operando: a Seção A acima é a fonte primária.",
                    agents.error
                ),
            );
            ui.add_space(theme::SPACE_XS);
        }
        if agents.list.is_empty() {
            kit::empty_state(ui, "Nenhum agente listado. Clique Atualizar Orquestrador.");
        } else {
            kit::table("agents_http_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "AGENTE",
                        "MODELO VINCULADO",
                        "TASKS CONCLUÍDAS",
                        "TASKS FALHAS",
                        "TAXA DE SUCESSO (%)",
                        "LATÊNCIA EMA",
                    ],
                );
                for agent in &agents.list {
                    ui.label(
                        egui::RichText::new(format!("● {}", agent.agent_id))
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    ui.label(
                        egui::RichText::new(&agent.model)
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    kit::num_cell(ui, &agent.completed_total.to_string());
                    kit::num_cell(ui, &agent.failed_total.to_string());
                    match success_rate(agent.completed_total, agent.failed_total) {
                        Some(rate) => {
                            ui.horizontal(|ui| {
                                ui.add(egui::ProgressBar::new(
                                    (rate as f32 / 100.0).clamp(0.0, 1.0),
                                ));
                                ui.label(
                                    egui::RichText::new(format!("{rate:.2}%"))
                                        .monospace()
                                        .small()
                                        .color(if rate >= 99.0 {
                                            theme::OK
                                        } else if rate >= 90.0 {
                                            theme::WARN
                                        } else {
                                            theme::ERROR
                                        }),
                                );
                            });
                        }
                        None => {
                            ui.label("—");
                        }
                    }
                    kit::num_cell(ui, &format!("{:.1} ms", agent.ema_latency_ms));
                    ui.end_row();
                }
            });
            ui.add_space(theme::SPACE_XS);
            // Linha agregada do enxame (mockup; derivações honestas).
            let completed: u64 = agents.list.iter().map(|a| a.completed_total).sum();
            let failed: u64 = agents.list.iter().map(|a| a.failed_total).sum();
            let models: std::collections::BTreeSet<&str> =
                agents.list.iter().map(|a| a.model.as_str()).collect();
            let ema_avg = agents.list.iter().map(|a| a.ema_latency_ms).sum::<f32>()
                / agents.list.len().max(1) as f32;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("TOTAL AGREGADO (ENXAME)")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                });
                ui.label(
                    egui::RichText::new(format!("{} Modelo(s) Ativo(s)", models.len()))
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(format!("{completed} completadas"))
                        .monospace()
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(format!("{failed} falhas"))
                        .monospace()
                        .strong()
                        .color(if failed == 0 { theme::OK } else { theme::ERROR }),
                );
                if let Some(rate) = success_rate(completed, failed) {
                    ui.horizontal(|ui| {
                        ui.add(egui::ProgressBar::new(
                            (rate as f32 / 100.0).clamp(0.0, 1.0),
                        ));
                        ui.label(
                            egui::RichText::new(format!("{rate:.2}% confiabilidade"))
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                }
                ui.label(
                    egui::RichText::new(format!("Média: {ema_avg:.0}ms"))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(format!("{} agente(s) no pool", agents.list.len()))
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
            });
        }
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Baixar Snapshot JSON").clicked() {
                    match agents.export_snapshot(&std::env::temp_dir()) {
                        Ok(path) => {
                            agents.notice = format!("snapshot exportado: {}", path.display());
                        }
                        Err(err) => {
                            agents.error = format!("falha ao exportar: {err}");
                        }
                    }
                }
                if ui.button("Limpar Estatísticas").clicked() {
                    agents.clear_stats();
                }
            });
        });
        if !agents.notice.is_empty() {
            ui.label(
                egui::RichText::new(&agents.notice)
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        }
    });
}
