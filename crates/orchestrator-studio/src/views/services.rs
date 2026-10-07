//! Painel 3.8 Serviços do Sistema (systemd): banner de intertravamento do
//! modo protegido, plano pretendido × efetivo em colunas uppercase, comandos
//! ▶ Iniciar ■ Parar ↺ Reiniciar (todos sob confirmação em 2 passos) e o log
//! de auditoria das atuações aplicadas pela GUI.

use crate::kit;
use crate::panel_header::panel_header;
use crate::protected::ProtectedGuard;
use crate::protected::ProtectedOutcome;
use crate::services::ServicesPanel;
use crate::theme;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, panel: &mut ServicesPanel, guard: &mut ProtectedGuard) {
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    panel.poll();

    let target = if panel.url.is_empty() {
        "—"
    } else {
        panel.url.as_str()
    };
    panel_header(
        ui,
        "SEC 3.8 · SERVIÇOS DO SISTEMA (SYSTEMD) · DAEMON CONTROL",
        "Serviços do Sistema",
        &format!(
            "ALVO: {target} · plano via GET /services (pretendido × efetivo) · \
             atuação remota: POST /services/<unidade>/<ação> sob confirmação"
        ),
    );
    kit::target_chip(
        ui,
        target,
        if panel.busy {
            "lendo plano…"
        } else if panel.list.is_empty() {
            "sem leitura"
        } else {
            "plano carregado"
        },
        !panel.list.is_empty(),
    );
    ui.add_space(theme::SPACE_SM);

    // ── Banner de intertravamento (mockup 3.8) — espelha o guard global ──
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
                ui.label(
                    egui::RichText::new("🛡 INTERTRAVAMENTO DE MODO PROTEGIDO")
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                if armed {
                    kit::badge(ui, "ARMADO (GRAVAÇÃO ATIVA)", theme::ERROR);
                } else {
                    kit::badge(ui, "DESARMADO (Leitura Somente)", theme::STALE);
                }
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
                if let Some(refusal) = &guard.last_refusal {
                    if !armed {
                        ui.label(egui::RichText::new(refusal).small().color(theme::WARN));
                    }
                }
            });
        });
    ui.add_space(theme::SPACE_MD);

    // ── Plano do nó (collapsing com âncora literal dos kittest) ──
    ui.collapsing("Serviços do nó (plano: pretendido × efetivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("nó:");
            ui.add_enabled_ui(!panel.busy, |ui| {
                ui.text_edit_singleline(&mut panel.url);
                if ui.button("Ler plano").clicked() {
                    panel.refresh();
                }
            });
            match panel.last_sync {
                Some(at) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "Última sincronização: há {} s",
                            at.elapsed().as_secs()
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                }
                None => {
                    ui.label(
                        egui::RichText::new("Última sincronização: —")
                            .monospace()
                            .small()
                            .color(theme::STALE),
                    );
                }
            }
        });
        if !panel.error.is_empty() {
            kit::error_banner(ui, &panel.error);
        }
        if panel.list.is_empty() {
            kit::empty_state(ui, "Nenhum serviço lido. Clique Ler plano.");
        } else {
            let divergent = panel
                .list
                .iter()
                .filter(|row| row.wanted.is_some_and(|wanted| wanted != row.active))
                .count();
            ui.label(
                egui::RichText::new(format!(
                    "UNIDADES DO NÓ ALVO: {} DECLARADAS · {} DIVERGÊNCIA(S)",
                    panel.list.len(),
                    divergent
                ))
                .monospace()
                .small()
                .color(if divergent == 0 {
                    theme::OK
                } else {
                    theme::WARN
                }),
            );
            ui.add_space(theme::SPACE_SM);
            egui::Grid::new("services_grid")
                .striped(true)
                .show(ui, |ui| {
                    kit::grid_header(
                        ui,
                        &[
                            "Unidade (Serviço)",
                            "Pretendido",
                            "Efetivo",
                            "Divergência / Diff",
                            "Comandos Remotos",
                        ],
                    );
                    let mut pending: Option<(String, u8)> = None; // 0=iniciar 1=parar 2=reiniciar
                    for row in &panel.list {
                        let wanted = match row.wanted {
                            Some(true) => "on",
                            Some(false) => "off",
                            None => "—",
                        };
                        let active = if row.active { "on" } else { "off" };
                        kit::mono_cell(ui, &row.service);
                        ui.label(wanted);
                        ui.label(egui::RichText::new(active).color(if row.active {
                            theme::OK
                        } else {
                            theme::STALE
                        }));
                        // Divergência honesta com o par pretendido×efetivo.
                        match row.wanted {
                            Some(w) if w != row.active => {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "⇐ DIVERGE (wanted={w}, active={})",
                                        row.active
                                    ))
                                    .monospace()
                                    .small()
                                    .color(theme::ERROR),
                                );
                            }
                            Some(false) if !row.active => {
                                ui.label(
                                    egui::RichText::new("◌ DESATIVADO (conforme pretendido)")
                                        .monospace()
                                        .small()
                                        .color(theme::ON_SURFACE_VARIANT),
                                );
                            }
                            _ => {
                                ui.label(
                                    egui::RichText::new("SINCRONIZADO")
                                        .monospace()
                                        .small()
                                        .color(theme::OK),
                                );
                            }
                        }
                        ui.add_enabled_ui(!panel.busy, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("▶ Iniciar").clicked() {
                                    pending = Some((row.service.clone(), 0));
                                }
                                if ui.button("■ Parar").clicked() {
                                    pending = Some((row.service.clone(), 1));
                                }
                                if ui.button("↺ Reiniciar").clicked() {
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

        // Confirmação da ação pendente (visível até resolvida) — 2 passos
        // com banner âmbar (mockup 3.8). Âncoras literais dos kittest.
        if let Some(pending) = &guard.pending {
            ui.separator();
            crate::kit::warn_banner(
                ui,
                &format!(
                    "CONFIRMAÇÃO EM 2 PASSOS — Ação solicitada: {}",
                    pending.description
                ),
            );
            ui.horizontal(|ui| {
                if ui.button("✔ Confirmar").clicked() {
                    guard.confirm_pending();
                    if let Some(service) = panel.pending_restart.take() {
                        panel.restart_row(&service);
                    } else if let Some((service, start)) = panel.pending_action.take() {
                        panel.actuate_row(&service, start);
                    }
                }
                if ui.button("✘ Cancelar").clicked() {
                    guard.cancel_pending();
                    panel.pending_action = None;
                    panel.pending_restart = None;
                }
            });
        }
    });
    ui.add_space(theme::SPACE_MD);

    // ── Painel de auditoria (tela 3.8): atuações aplicadas pela GUI ──
    if !panel.audit.is_empty() {
        ui.strong("Painel de Auditoria & Log de Operações Systemd");
        ui.label(
            egui::RichText::new("TRANSPORTE: HTTP/1.1 POST /api/v1/systemd/unit")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        ui.add_space(theme::SPACE_XS);
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
                        egui::RichText::new(format!("→ {} · {}", entry.action, entry.outcome))
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                }
            });
    }
}
