//! Painel 3.4 Subir Inferência: fiel ao mockup Stitch — runner LOCAL de
//! subprocesso (spawn direto, sem systemd), parâmetros + presets, CLI
//! preview, status/PID/VRAM reais, wizard das 5 etapas com tempos medidos,
//! console stdout/stderr ao vivo e chip ServerStatus do domínio.
//!
//! Só dados reais: PID/saída/exit do filho, `/health` medido, VRAM do hwmon
//! AMD quando existe (ausente = "indisponível"), SHA-256 calculado em worker.
//! Desvios honestos do mockup: sem cgroup/systemd (spawn direto), bind padrão
//! em loopback (0.0.0.0 expõe inferência sem token), sem GUID/build
//! inventados (build vem do `--version` real).

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::runner::{fmt_gb, fmt_uptime, Preset, RunnerState, RunnerStatus};
use crate::theme;
use eframe::egui;

/// Moldura dos cards da 3.4 (mesmo idioma das demais).
/// `go_chat` vira `true` quando o operador pede "Abrir no Chat (3.3)".
pub fn show(
    ui: &mut egui::Ui,
    runner: &mut RunnerState,
    go_chat: &mut bool,
    discovery: &DiscoveryState,
) {
    runner.poll();
    runner.ensure_build();
    runner.ensure_sha();
    if runner.running() {
        ui.ctx().request_repaint();
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
    }

    // ── Cabeçalho (mockup; shell global novo já vem do main) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("3.4 Subir Inferência")
                .size(22.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        kit::badge(ui, "RUNNER LOCAL", theme::PRIMARY_FIXED_DIM);
    });
    ui.label(
        egui::RichText::new(
            "Gerenciamento de subprocesso local do llama-server (spawn direto) & publicação de status DDS",
        )
        .small()
        .color(theme::ON_SURFACE_VARIANT),
    );
    ui.add_space(theme::SPACE_SM);

    // ── Faixa de estado: subprocesso · acelerador · VRAM ──
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.columns(3, |cols| {
            cols[0].vertical(|ui| {
                ui.label(
                    egui::RichText::new("SUBPROCESS STATE")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(match &runner.status {
                    RunnerStatus::Stopped => egui::RichText::new("○ PARADO")
                        .monospace()
                        .strong()
                        .color(theme::STALE),
                    RunnerStatus::Starting => egui::RichText::new("○ SUBINDO…")
                        .monospace()
                        .strong()
                        .color(theme::WARN),
                    RunnerStatus::Running { pid, since, .. } => egui::RichText::new(format!(
                        "● PID {pid} (ATIVO · {})",
                        fmt_uptime(since.elapsed().as_secs())
                    ))
                    .monospace()
                    .strong()
                    .color(theme::OK),
                    RunnerStatus::Stopping { pid, .. } => {
                        egui::RichText::new(format!("○ PID {pid} (PARANDO…)"))
                            .monospace()
                            .strong()
                            .color(theme::WARN)
                    }
                    RunnerStatus::Failed { .. } => egui::RichText::new("× FALHOU")
                        .monospace()
                        .strong()
                        .color(theme::ERROR),
                });
            });
            cols[1].vertical(|ui| {
                ui.label(
                    egui::RichText::new("TARGET ACCELERATOR")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new(runner.gpu_label.as_deref().unwrap_or("CPU (sem GPU DRI)"))
                        .monospace()
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
            });
            cols[2].vertical(|ui| {
                ui.label(
                    egui::RichText::new("VRAM ALOCADA")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                match runner.vram {
                    Some((used, total)) if total > 0 => {
                        let frac = (used as f32 / total as f32).clamp(0.0, 1.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {} GB  {:.1}%",
                                fmt_gb(used),
                                fmt_gb(total),
                                frac * 100.0
                            ))
                            .monospace()
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.add(egui::ProgressBar::new(frac));
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new("indisponível (sem hwmon)")
                                .monospace()
                                .small()
                                .color(theme::OUTLINE),
                        );
                    }
                }
            });
        });
    });
    ui.add_space(theme::SPACE_LG);

    ui.columns(2, |cols| {
        show_params(cols, runner, go_chat, discovery);
        show_status(cols, runner, go_chat, discovery);
    });
}

mod launch_helpers;
mod launch_params;
mod launch_status;

use launch_helpers::{card_frame, card_title, num_field};
use launch_params::show_params;
use launch_status::show_status;
