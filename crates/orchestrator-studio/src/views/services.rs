//! Painel de serviços: plano legível pretendido × efetivo, com ACTUAÇÃO de
//! unidades remotas protegida pelo modo protegido (T-890-08, G-38/65).

use crate::protected::ProtectedGuard;
use crate::protected::ProtectedOutcome;
use crate::services::ServicesPanel;
use eframe::egui;

/// URL, botão de leitura, erro e tabela com a divergência em destaque.
pub fn show(ui: &mut egui::Ui, panel: &mut ServicesPanel, guard: &mut ProtectedGuard) {
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    panel.poll();
    ui.collapsing("Serviços do nó (plano: pretendido × efetivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("nó:");
            ui.text_edit_singleline(&mut panel.url);
            ui.add_enabled_ui(!panel.busy, |ui| {
                if ui.button("Ler plano").clicked() {
                    panel.refresh();
                }
            });
        });
        if !panel.error.is_empty() {
            ui.label(&panel.error);
        }
        if panel.list.is_empty() {
            ui.label("Nenhum serviço lido. Clique Ler plano.");
        } else {
            egui::Grid::new("services_grid").show(ui, |ui| {
                ui.label("serviço");
                ui.label("pretendido");
                ui.label("efetivo");
                ui.label("diff");
                ui.label("ação");
                ui.end_row();
                let mut pending: Option<(String, bool)> = None;
                for row in &panel.list {
                    let wanted = match row.wanted {
                        Some(true) => "on",
                        Some(false) => "off",
                        None => "—",
                    };
                    let active = if row.active { "on" } else { "off" };
                    let diff = match row.wanted {
                        Some(w) if w == row.active => "",
                        _ => "⇐ diverge",
                    };
                    ui.label(&row.service);
                    ui.label(wanted);
                    ui.label(active);
                    ui.label(diff);
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!panel.busy, |ui| {
                            if ui.button("▶").clicked() {
                                pending = Some((row.service.clone(), true));
                            }
                            if ui.button("■").clicked() {
                                pending = Some((row.service.clone(), false));
                            }
                        });
                    });
                    ui.end_row();
                }
                if let Some((service, start)) = pending {
                    let description = format!(
                        "{} serviço '{}' no nó",
                        if start { "INICIAR" } else { "PARAR" },
                        service
                    );
                    // G-38/65: desarmado RECUSA (nada trafega); armado pede
                    // confirmação explícita antes de tocar a máquina remota.
                    match guard.request(&description) {
                        ProtectedOutcome::NeedsConfirmation(_) => {
                            panel.pending_action = Some((service.clone(), start));
                        }
                        ProtectedOutcome::Refused(_) => {}
                    }
                }
            });
        }
        // Confirmação da ação pendente (visível até resolvida).
        if let Some(pending) = &guard.pending {
            ui.separator();
            ui.colored_label(
                egui::Color32::YELLOW,
                format!("CONFIRMAR: {}", pending.description),
            );
            ui.horizontal(|ui| {
                if ui.button("✔ Confirmar").clicked() {
                    guard.confirm_pending();
                    if let Some((service, start)) = panel.pending_action.take() {
                        panel.actuate_row(&service, start);
                    }
                }
                if ui.button("✘ Cancelar").clicked() {
                    guard.cancel_pending();
                    panel.pending_action = None;
                }
            });
        }
    });
}
