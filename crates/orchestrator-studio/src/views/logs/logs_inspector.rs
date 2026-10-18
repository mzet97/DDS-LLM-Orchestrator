//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_inspector(ui: &mut egui::Ui, panel: &mut LogsPanel, live: &[LogEntry]) {
    // ── Inspetor forense da entrada selecionada ──
    let shown: &[LogEntry] = if panel.paused { &panel.frozen } else { live };
    if let Some(slot) = panel.selected {
        if let Some(entry) = shown.iter().find(|e| e.slot == slot) {
            ui.add_space(theme::SPACE_SM);
            section(ui, &format!("INSPETOR DIAGNÓSTICO · SLOT #{}", entry.slot));
            kit::accent_card(ui, level_color(entry.level), |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "THREAD: {} · FONTE: {}",
                            entry.thread, entry.origin
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("fechar").monospace().small(),
                            ))
                            .clicked()
                        {
                            panel.selected = None;
                        }
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("COPIAR DUMP").monospace().small(),
                            ))
                            .clicked()
                        {
                            ui.ctx().copy_text(dump(entry));
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "SUBSISTEMA: [{}] · MOMENTO: {} · OFFSET: {}",
                            entry.source,
                            entry.timestamp,
                            fmt_offset(rel_of(shown, entry.slot))
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.add_space(theme::SPACE_XS);
                ui.label(header("RESUMO DA MENSAGEM RAW"));
                ui.add(
                    egui::TextEdit::multiline(&mut entry.message.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(2)
                        .interactive(false),
                );
                if let Some(payload) = &entry.payload {
                    ui.add_space(theme::SPACE_XS);
                    ui.label(header("CONTEXTO SERIALIZADO DO NÓ (JSON)"));
                    ui.add(
                        egui::TextEdit::multiline(&mut payload.clone())
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(3)
                            .interactive(false),
                    );
                }
                if let Some(backtrace) = &entry.backtrace {
                    ui.add_space(theme::SPACE_XS);
                    ui.label(header("CALL STACK TRACE (RUST · CAPTURADO NO REGISTRO)"));
                    egui::ScrollArea::vertical()
                        .id_salt("log-backtrace")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut backtrace.clone())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(6)
                                    .interactive(false),
                            );
                        });
                }
            });
        }
    }
}
