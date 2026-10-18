//! Parte de `views::services` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_plan(
    ui: &mut egui::Ui,
    panel: &mut ServicesPanel,
    guard: &mut ProtectedGuard,
    _discovery: &DiscoveryState,
) {
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
}
