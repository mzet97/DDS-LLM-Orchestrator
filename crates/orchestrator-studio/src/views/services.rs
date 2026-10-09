//! Painel 3.8 Serviços do Sistema (systemd): alvo + sincronização, banner
//! de intertravamento do modo protegido, confirmação em 2 passos, plano
//! pretendido × efetivo com comandos remotos e auditoria com relógio.
//!
//! Só dados reais: o fio entrega `{service, wanted, active}` — sem PID,
//! subestado, enabled, descrição ou versão do init (itens do mockup que não
//! existem e ficam FORA). Transporte real: `POST /services/<u>/<start|stop>`.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::panel_header::panel_header;
use crate::protected::ProtectedGuard;
use crate::protected::ProtectedOutcome;
use crate::services::ServicesPanel;
use crate::theme;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    panel: &mut ServicesPanel,
    guard: &mut ProtectedGuard,
    discovery: &DiscoveryState,
) {
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    panel.poll();

    let target = if panel.url.is_empty() {
        String::from("—")
    } else {
        panel.url.clone()
    };
    panel_header(
        ui,
        "SEC 3.8",
        "3.8 Serviços do Sistema (systemd)",
        "Daemon control no nó alvo: plano pretendido × efetivo e atuação remota sob intertravamento.",
    );
    ui.horizontal(|ui| {
        kit::badge(ui, "DAEMON CONTROL", theme::PRIMARY_FIXED_DIM);
        ui.label(
            egui::RichText::new(format!(
                "ALVO: {target} · ● DDS Domain {}",
                discovery.domain
            ))
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let read = ui.add_enabled(!panel.busy, egui::Button::new("↻ Ler plano de serviços"));
            if read.clicked() {
                panel.audit_manual_list = true; // GET manual → auditoria
                panel.refresh();
            }
            ui.label(
                egui::RichText::new(match panel.last_sync_ms {
                    Some(ms) => format!("Última Sincronização: {}", kit::clock_ms(ms)),
                    None => String::from("Última Sincronização: —"),
                })
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        ui.label(caption("NÓ ALVO (HTTP :4317)"));
        ui.add_enabled_ui(!panel.busy, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut panel.url)
                    .desired_width(300.0)
                    .hint_text("http://127.0.0.1:4317"),
            );
        });
    });
    if !panel.error.is_empty()
        && panel.error != "lendo plano…"
        && panel.error != "atuando…"
        && !panel.error.starts_with("reiniciando")
    {
        kit::error_banner(ui, &panel.error);
    } else if panel.busy {
        ui.label(
            egui::RichText::new(format!("○ {}", panel.error))
                .small()
                .color(theme::WARN),
        );
    }
    ui.add_space(theme::SPACE_SM);

    // ── Banner de intertravamento — espelha o guard global ──
    let armed = guard.armed;
    let (banner_fill, banner_stroke) = if armed {
        (theme::tint(theme::ERROR, 10), theme::ERROR)
    } else {
        (theme::SURFACE_HIGH, theme::SURFACE_HIGHEST)
    };
    egui::Frame::NONE
        .fill(banner_fill)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, banner_stroke))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("INTERTRAVAMENTO DE MODO PROTEGIDO")
                                .strong()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        if armed {
                            kit::badge(ui, "ARMADO (GRAVAÇÃO ATIVA)", theme::ERROR);
                        } else {
                            kit::badge(ui, "DESARMADO (LEITURA SOMENTE)", theme::STALE);
                        }
                    });
                    ui.label(
                        egui::RichText::new(
                            "Comandos de mutação remota (start, stop, restart) no nó via \
                             systemd requerem armamento explícito do intertravamento.",
                        )
                        .small()
                        .weak(),
                    );
                    if let Some(refusal) = &guard.last_refusal {
                        if !armed {
                            ui.label(egui::RichText::new(refusal).small().color(theme::WARN));
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle = ui.add(egui::Button::new(
                        egui::RichText::new(if armed {
                            "DISARM / PROTEGER NÓ"
                        } else {
                            "ARM / HABILITAR ATUAÇÃO"
                        })
                        .monospace()
                        .small()
                        .color(if armed {
                            theme::ON_PRIMARY
                        } else {
                            theme::WARN
                        })
                        .strong(),
                    ));
                    if toggle.clicked() {
                        guard.set_armed(!armed);
                    }
                });
            });
        });
    ui.add_space(theme::SPACE_SM);

    // ── Confirmação da ação pendente (fora do collapsing: banner vermelho
    //    em largura total — âncora literal preservada) ──
    if let Some(pending) = &guard.pending {
        crate::kit::danger_banner(
            ui,
            &format!(
                "CONFIRMAÇÃO EM 2 PASSOS — Ação solicitada: {}",
                pending.description
            ),
        );
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("× Cancelar").clicked() {
                    guard.cancel_pending();
                    panel.pending_action = None;
                    panel.pending_restart = None;
                }
                if ui.button("✓ Confirmar").clicked() {
                    guard.confirm_pending();
                    if let Some(service) = panel.pending_restart.take() {
                        panel.restart_row(&service);
                    } else if let Some((service, start)) = panel.pending_action.take() {
                        panel.actuate_row(&service, start);
                    }
                }
            });
        });
        ui.add_space(theme::SPACE_SM);
    }

    // ── Plano do nó (collapsing com âncora literal dos kittest) ──
    ui.collapsing("Serviços do nó (plano: pretendido × efetivo)", |ui| {
        if panel.list.is_empty() {
            kit::empty_state(
                ui,
                "Nenhum serviço lido. Clique em ↻ Ler plano de serviços acima.",
            );
        } else {
            let divergent = panel
                .list
                .iter()
                .filter(|row| row.wanted.is_some_and(|wanted| wanted != row.active))
                .count();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "UNIDADES DO NÓ ALVO: {} Declaradas no Plano",
                        panel.list.len()
                    ))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                kit::badge(
                    ui,
                    &format!(
                        "{} DIVERGÊNCIA{}",
                        divergent,
                        if divergent == 1 { "" } else { "S" }
                    ),
                    if divergent == 0 {
                        theme::OK
                    } else {
                        theme::ERROR
                    },
                );
            });
            ui.add_space(theme::SPACE_SM);
            kit::table("services_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "Unidade (Serviço)",
                        "Estado Pretendido",
                        "Estado Efetivo",
                        "Divergência / Diff",
                        "Comandos Remotos",
                    ],
                );
                let mut pending: Option<(String, u8)> = None; // 0=iniciar 1=parar 2=reiniciar
                for row in &panel.list {
                    let wanted_text = match row.wanted {
                        Some(true) => "Wanted: true",
                        Some(false) => "Wanted: false",
                        None => "Wanted: —",
                    };
                    kit::mono_cell(ui, &row.service);
                    ui.label(
                        egui::RichText::new(wanted_text)
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.label(
                        egui::RichText::new(if row.active {
                            "Active: true"
                        } else {
                            "Active: false"
                        })
                        .monospace()
                        .small()
                        .color(if row.active {
                            theme::OK
                        } else {
                            theme::STALE
                        }),
                    );
                    // Divergência honesta com o par pretendido×efetivo.
                    match row.wanted {
                        Some(w) if w != row.active => {
                            ui.label(
                                egui::RichText::new(format!(
                                    "● DIVERGE (wanted={w}, active={})",
                                    row.active
                                ))
                                .monospace()
                                .small()
                                .color(theme::ERROR),
                            );
                        }
                        Some(false) if !row.active => {
                            ui.label(
                                egui::RichText::new("○ DESATIVADO (conforme pretendido)")
                                    .monospace()
                                    .small()
                                    .color(theme::ON_SURFACE_VARIANT),
                            );
                        }
                        _ => {
                            ui.label(
                                egui::RichText::new("● SINCRONIZADO (Ok)")
                                    .monospace()
                                    .small()
                                    .color(theme::OK),
                            );
                        }
                    }
                    // Ação corretiva sugerida: INICIAR acende quando o plano
                    // quer ativo e está morto; PARAR quando quer parado e
                    // está vivo (derivado do par — sem chute).
                    let fix_start = row.wanted == Some(true) && !row.active;
                    let fix_stop = row.wanted == Some(false) && row.active;
                    ui.add_enabled_ui(!panel.busy, |ui| {
                        ui.horizontal(|ui| {
                            let start = ui.add(cmd_button(fix_start, "▶ Iniciar"));
                            if start.clicked() {
                                pending = Some((row.service.clone(), 0));
                            }
                            let stop = ui.add(cmd_button(fix_stop, "■ Parar"));
                            if stop.clicked() {
                                pending = Some((row.service.clone(), 1));
                            }
                            if ui.button("↻ Reiniciar").clicked() {
                                pending = Some((row.service.clone(), 2));
                            }
                        });
                    });
                    ui.end_row();
                }
                if let Some((service, action)) = pending {
                    let description = format!(
                        "{} serviço '{service}' no nó",
                        match action {
                            0 => "INICIAR",
                            1 => "PARAR",
                            _ => "REINICIAR (stop→start)",
                        }
                    );
                    // G-38/65: desarmado RECUSA (nada trafega); armado pede
                    // confirmação explícita antes de tocar a máquina remota.
                    match guard.request(&description) {
                        ProtectedOutcome::NeedsConfirmation(_) => {
                            if action == 2 {
                                panel.pending_restart = Some(service);
                            } else {
                                panel.pending_action = Some((service, action == 0));
                            }
                        }
                        ProtectedOutcome::Refused(_) => {}
                    }
                }
            });
        }
    });
    ui.add_space(theme::SPACE_MD);

    // ── Painel de auditoria: atuações + GETs manuais, com relógio ──
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Painel de Auditoria & Log de Operações Systemd").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clear = ui.add_enabled(
                !panel.audit.is_empty(),
                egui::Button::new("Limpar Visualização"),
            );
            if clear.clicked() {
                panel.audit.clear();
            }
            ui.label(
                egui::RichText::new("TRANSPORTE: POST /services/<unidade>/<start|stop>")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    if panel.audit.is_empty() {
        kit::empty_state(ui, "nenhuma atuação registrada nesta sessão.");
    } else {
        egui::Frame::NONE
            .fill(theme::SURFACE_LOW)
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                // Mais recente primeiro (terminal).
                for entry in panel.audit.iter().rev().take(10) {
                    ui.label(
                        egui::RichText::new(format!(
                            "[{}] {} → {}",
                            kit::clock_ms(entry.ts_ms),
                            entry.action,
                            entry.outcome
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                }
            });
    }
}

/// Botão de comando: destaque ciano quando é a correção sugerida.
fn cmd_button(highlight: bool, text: &str) -> egui::Button<'_> {
    let label = if highlight {
        egui::RichText::new(text)
            .monospace()
            .small()
            .color(theme::ON_PRIMARY)
            .strong()
    } else {
        egui::RichText::new(text).monospace().small()
    };
    let mut button = egui::Button::new(label);
    if highlight {
        button = button
            .fill(theme::PRIMARY_CONTAINER)
            .stroke(egui::Stroke::NONE);
    }
    button
}

fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}
