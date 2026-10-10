//! Painel 3.5 Agentes: fiel ao mockup Stitch — duas fontes com semântica
//! distinta (FONTE 1: DDS/AgentRegistry ao vivo × FONTE 2: orquestrador HTTP
//! auxiliar), Seção A com barras de slots/lease, Seção B com taxas reais,
//! agregado do enxame e snapshot JSON.
//!
//! Só dados reais: sem GUID/NIC/MTU/p95/jitter de agente (o fio não tem),
//! sem estratégia de despacho por agente (não anunciada), sem fila global ou
//! policy (inexistentes no contrato). O QoS `TRANSIENT_LOCAL / RELIABLE` e a
//! poda de 30 s vêm do CÓDIGO (`qos.rs`, `discovery.rs`). O `TOPIC_ID` hexa
//! do mockup é floreio inventado — omitido.

use crate::agents::AgentsState;
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::now_unix_ns;
use crate::theme;
use eframe::egui;

/// Taxa de sucesso honesta: concluídas/(concluídas+falhas); `None` sem dados.
fn success_rate(completed: u64, failed: u64) -> Option<f64> {
    let total = completed + failed;
    (total > 0).then(|| completed as f64 / total as f64 * 100.0)
}

/// Instante ms unix → "14:02:11.412" (UTC, conta manual).
fn hhmmss_ms(ts_unix_ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (ts_unix_ms / 3_600_000) % 24,
        (ts_unix_ms / 60_000) % 60,
        (ts_unix_ms / 1_000) % 60,
        ts_unix_ms % 1_000
    )
}

/// Moldura dos cards da 3.5 (mesmo idioma das demais).
fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

pub fn show(ui: &mut egui::Ui, agents: &mut AgentsState, discovery: &DiscoveryState) {
    agents.poll();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));

    // ── Cabeçalho (mockup) + fontes ──
    ui.horizontal(|ui| {
        kit::badge(ui, "SEC 3.5 // ENXAME DE AGENTES", theme::PRIMARY_FIXED_DIM);
        ui.label(
            egui::RichText::new(
                "Topologia Híbrida: RTPS Multicast Mesh · Orquestrador de Despacho (:8080)",
            )
            .size(15.0)
            .strong()
            .color(theme::ON_SURFACE),
        );
    });
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(if discovery.scanning {
                "○ FONTE 1: DDS RTPS (DESCOBRINDO…)"
            } else {
                "● FONTE 1: DDS RTPS (DISCOVERY OK)"
            })
            .monospace()
            .small()
            .strong()
            .color(if discovery.scanning {
                theme::WARN
            } else {
                theme::OK
            }),
        );
        let (fonte2, fonte2_color) = if agents.busy {
            ("○ FONTE 2: HTTP :8080 (LENDO…)", theme::WARN)
        } else if agents.last_ok && agents.error.is_empty() {
            ("● FONTE 2: HTTP :8080 (SYNCED)", theme::OK)
        } else if agents.error.is_empty() {
            ("○ FONTE 2: HTTP :8080 (NÃO LIDO)", theme::STALE)
        } else {
            ("○ FONTE 2: HTTP :8080 (FALHOU)", theme::WARN)
        };
        ui.label(
            egui::RichText::new(fonte2)
                .monospace()
                .small()
                .strong()
                .color(fonte2_color),
        );
        if ui
            .selectable_label(agents.simulate_outage, "Simular Queda :8080")
            .clicked()
        {
            agents.toggle_outage();
        }
    });
    ui.add_space(theme::SPACE_XS);
    ui.label(
        egui::RichText::new(
            "SEMÂNTICA DUAL: os agentes descobertos via RTPS operam em barramento \
             peer-to-peer autônomo (alta tolerância). A telemetria HTTP é auxiliar \
             e NÃO introduz ponto único de falha. QoS: TRANSIENT_LOCAL / RELIABLE",
        )
        .small()
        .color(theme::ON_SURFACE_VARIANT),
    );
    ui.add_space(theme::SPACE_LG);

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

#[cfg(test)]
mod tests {
    use super::{hhmmss_ms, success_rate};

    #[test]
    fn polling_clock_formats_utc_with_millis() {
        assert_eq!(hhmmss_ms(0), "00:00:00.000");
        assert_eq!(hhmmss_ms(3_661_005), "01:01:01.005");
    }

    #[test]
    fn success_rate_needs_completed_or_failed() {
        assert_eq!(success_rate(0, 0), None);
        assert!((success_rate(3, 1).unwrap() - 75.0).abs() < f64::EPSILON);
    }
}
