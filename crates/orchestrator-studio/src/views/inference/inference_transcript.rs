//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_transcript(
    ui: &mut egui::Ui,
    infer: &mut InferenceState,
    _discovery: &DiscoveryState,
) {
    // ── Transcript de engenharia & depuração (mockup) ──
    let (session_toks, turns) = session_tokens(infer);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("TRANSCRIPT DE ENGENHARIA & DEPURAÇÃO")
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.label(
            egui::RichText::new(if turns == 0 {
                String::from("SESSÃO VAZIA")
            } else {
                format!("TOKENS NA SESSÃO (USAGE REAL): {session_toks} · {turns} turno(s)")
            })
            .monospace()
            .small()
            .color(theme::PRIMARY_FIXED_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Exportar JSON").clicked() {
                match infer.export_session(&std::env::temp_dir()) {
                    Ok(path) => {
                        infer.reply = format!("transcript exportado: {}", path.display());
                        crate::studio_log::info(format!(
                            "inferência: transcript exportado para {}",
                            path.display()
                        ));
                    }
                    Err(err) => {
                        infer.reply = format!("falha ao exportar: {err}");
                    }
                }
            }
            if ui.button("Limpar Histórico").clicked() {
                infer.clear_session();
            }
        });
    });
    ui.add_space(theme::SPACE_XS);
    egui::ScrollArea::vertical()
        .id_salt("infer-transcript")
        .max_height(380.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if infer.history.is_empty() {
                kit::empty_state(ui, "sessão vazia — envie o primeiro prompt abaixo.");
            }
            let mut assistant_index = 0usize;
            for message in &infer.history {
                match message.role {
                    Role::System => {
                        ui.label(
                            egui::RichText::new(format!("sistema: {}", message.content)).weak(),
                        );
                    }
                    Role::User => {
                        card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                kit::badge(ui, "OP", theme::PRIMARY_FIXED_DIM);
                                ui.label(
                                    egui::RichText::new("você (operador local)")
                                        .small()
                                        .strong()
                                        .color(theme::ON_SURFACE),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(hhmmss_utc(message.ts_unix_ms))
                                                .monospace()
                                                .small()
                                                .color(theme::OUTLINE),
                                        );
                                    },
                                );
                            });
                            ui.label(
                                egui::RichText::new(&message.content).color(theme::ON_SURFACE),
                            );
                        });
                        ui.add_space(theme::SPACE_XS);
                    }
                    Role::Assistant => {
                        let stats = infer.stats.get(assistant_index).cloned();
                        assistant_index += 1;
                        card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                kit::badge(ui, "SRV", theme::PRIMARY_CONTAINER);
                                ui.label(
                                    egui::RichText::new(if infer.model.is_empty() {
                                        String::from("assistente")
                                    } else {
                                        format!("assistente ({})", infer.model)
                                    })
                                    .small()
                                    .strong()
                                    .color(theme::ON_SURFACE),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let content = message.content.clone();
                                        if ui.small_button("Copiar").clicked() {
                                            ui.ctx().copy_text(content);
                                        }
                                        if let Some(stats) = &stats {
                                            let tokens = match (
                                                stats.prompt_tokens,
                                                stats.completion_tokens,
                                            ) {
                                                (Some(p), Some(c)) => {
                                                    format!(" · {p}+{c} tokens")
                                                }
                                                (None, Some(c)) => {
                                                    format!(" · {c} tokens de saída")
                                                }
                                                _ => String::new(),
                                            };
                                            let tps = stats
                                                .tokens_per_sec()
                                                .map(|v| format!(" · {v:.1} tok/s"))
                                                .unwrap_or_default();
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{} · {}ms{tokens}{tps}",
                                                    hhmmss_utc(message.ts_unix_ms),
                                                    stats.elapsed_ms
                                                ))
                                                .monospace()
                                                .small()
                                                .color(theme::PRIMARY_FIXED_DIM),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(hhmmss_utc(message.ts_unix_ms))
                                                    .monospace()
                                                    .small()
                                                    .color(theme::OUTLINE),
                                            );
                                        }
                                    },
                                );
                            });
                            ui.label(
                                egui::RichText::new(&message.content).color(theme::SECONDARY_FIXED),
                            );
                        });
                        ui.add_space(theme::SPACE_XS);
                    }
                }
            }
        });
    ui.add_space(theme::SPACE_SM);
}
