//! Parte de `views::models` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_progress(ui: &mut egui::Ui, state: &mut ModelsState) {
    // ── Banner de progresso do worker real (bytes/taxa/ETA medidos) ──
    // Cancelar sai do empréstimo (E0500): marca aqui, executa depois.
    let mut cancel_clicked = false;
    if let Some(progress) = &state.hashing {
        let pct = progress.current_fraction() * 100.0;
        kit::accent_card(ui, theme::PRIMARY_CONTAINER, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "● CALCULANDO SHA-256 ({} de {} arquivos verificados)…",
                        progress.done, progress.total
                    ))
                    .strong()
                    .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new("× Cancelar").color(theme::ERROR))
                        .clicked()
                    {
                        cancel_clicked = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Arquivo corrente: {} ({} / {pct:.0}% processado)",
                        progress.current,
                        fmt_gb2(progress.current_total),
                    ))
                    .small(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "IO: {:.1} MB/s · ETA: {}",
                            progress.rate_bps as f64 / (1024.0 * 1024.0),
                            progress
                                .current_eta_secs()
                                .map(fmt_hhmmss)
                                .unwrap_or_else(|| String::from("—")),
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });
            // Largura finita: INFINITY gera NaN no layout (assert egui).
            let bar_width = ui.available_width();
            ui.add(
                egui::ProgressBar::new(progress.current_fraction())
                    .desired_width(bar_width)
                    .fill(theme::PRIMARY_CONTAINER),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Chunk: {} / {} (sha2 software, blocos de 64 KiB, 1 thread)",
                        fmt_gb2(progress.current_bytes),
                        fmt_gb2(progress.current_total),
                    ))
                    .small()
                    .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{pct:.1}%"))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                });
            });
        });
        ui.add_space(theme::SPACE_SM);
    }
    if cancel_clicked {
        state.cancel();
    }

    // Estado do manifesto (âncora literal: "manifesto carregado: N registro(s)").
    match (&state.manifest, state.manifest_error.is_empty()) {
        (Some(manifest), true) => {
            ui.label(
                egui::RichText::new(format!(
                    "manifesto carregado: {} registro(s)",
                    manifest.len()
                ))
                .monospace()
                .small()
                .color(theme::OK),
            );
        }
        (None, true) => {
            ui.label(
                egui::RichText::new("Sem manifesto carregado: os SHAs ficam sem cruzamento.")
                    .monospace()
                    .small()
                    .color(theme::STALE),
            );
        }
        _ => {
            kit::error_banner(ui, &state.manifest_error);
        }
    }
    ui.add_space(theme::SPACE_MD);
}
