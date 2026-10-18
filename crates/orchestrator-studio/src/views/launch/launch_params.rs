//! Parte de `views::launch` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_params(
    cols: &mut [egui::Ui],
    runner: &mut RunnerState,
    _go_chat: &mut bool,
    _discovery: &DiscoveryState,
) {
    // ── Parâmetros de inicialização (mockup) ──
    cols[0].vertical(|ui| {
        card_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            card_title(
                ui,
                "Parâmetros de Inicialização",
                "SPAWN DIRETO · SEM SYSTEMD",
            );
            ui.add_space(theme::SPACE_XS);
            ui.label(
                egui::RichText::new("EXECUTÁVEL LLAMA-SERVER")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!runner.running(), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut runner.params.exe)
                            .desired_width(ui.available_width() - 92.0)
                            .hint_text("/usr/bin/llama-server"),
                    );
                    if ui.button("Detectar").clicked() {
                        if let Some(exe) = crate::runner::detect_exe() {
                            runner.params.exe = exe;
                        } else {
                            runner.error = String::from("nenhum llama-server no PATH conhecido");
                        }
                    }
                });
            });
            ui.label(
                egui::RichText::new(if runner.build_line.is_empty() {
                    String::from("build: —")
                } else {
                    format!("build: {}", runner.build_line)
                })
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
            );
            ui.add_space(theme::SPACE_XS);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("ARTEFATO DE MODELO GGUF (DISCO)")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Recarregar").clicked() {
                        runner.refresh_models();
                    }
                });
            });
            if runner.models.is_empty() {
                ui.label(
                    egui::RichText::new(if runner.models_error.is_empty() {
                        "sem modelos no diretório".to_owned()
                    } else {
                        runner.models_error.clone()
                    })
                    .small()
                    .weak(),
                );
            } else {
                let current = std::path::Path::new(&runner.params.model)
                    .file_name()
                    .map(|raw| raw.to_string_lossy().into_owned())
                    .unwrap_or_default();
                egui::ComboBox::from_id_salt("runner-model")
                    .selected_text(if current.is_empty() {
                        "— escolha o GGUF —".to_owned()
                    } else {
                        current
                    })
                    .show_ui(ui, |ui| {
                        for artifact in &runner.models {
                            let label = format!(
                                "{} ({:.2} GB)",
                                artifact.file_name,
                                artifact.size_bytes as f64 / 1_000_000_000.0
                            );
                            let path = format!("{}/{}", runner.models_dir, artifact.file_name);
                            ui.selectable_value(&mut runner.params.model, path, label);
                        }
                    });
            }
            ui.label(
                egui::RichText::new(format!("Path: {}", runner.params.model))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(
                egui::RichText::new(if runner.sha_busy {
                    String::from("SHA-256: calculando…")
                } else if let Some(hex) = &runner.sha_hex {
                    format!("SHA-256 OK {}…", &hex[..hex.len().min(12)])
                } else {
                    String::from("SHA-256: selecione o modelo")
                })
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
            );
            ui.add_space(theme::SPACE_XS);
            ui.label(
                egui::RichText::new("PRESETS DE PERFIL OPERACIONAL")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.horizontal(|ui| {
                for (preset, title, sub) in [
                    (Preset::Precisao, "Alta Precisão", "Ctx 32k · Offload 0"),
                    (Preset::Latencia, "Baixa Latência", "Ctx 8k · Batch 1024"),
                    (
                        Preset::Agente,
                        "Codificação / Agente",
                        "Ctx 32k · 8 Threads",
                    ),
                ] {
                    let selected = runner.active_preset == Some(preset);
                    if ui
                        .selectable_label(selected, format!("{title}\n{sub}"))
                        .clicked()
                    {
                        runner.apply_preset(preset);
                    }
                }
            });
            ui.add_space(theme::SPACE_XS);
            ui.columns(2, |fields| {
                fields[0].vertical(|ui| {
                    let mut raw = runner.params.port.to_string();
                    ui.label(
                        egui::RichText::new("PORTA HTTP")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    if ui
                        .add(egui::TextEdit::singleline(&mut raw).desired_width(90.0))
                        .changed()
                    {
                        if let Ok(port) = raw.trim().parse::<u16>() {
                            runner.params.port = port;
                        }
                    }
                    if num_field(ui, &mut runner.params.gpu_layers, "GPU LAYERS") {
                        runner.active_preset = None;
                    }
                    ui.label(
                        egui::RichText::new("(0 = CPU puro)")
                            .small()
                            .color(theme::OUTLINE),
                    );
                    let mut threads_raw = runner.params.threads.to_string();
                    ui.label(
                        egui::RichText::new("CPU THREADS")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    if ui
                        .add(egui::TextEdit::singleline(&mut threads_raw).desired_width(90.0))
                        .changed()
                    {
                        if let Ok(threads) = threads_raw.trim().parse::<u32>() {
                            runner.params.threads = threads;
                            runner.active_preset = None;
                        }
                    }
                });
                fields[1].vertical(|ui| {
                    if num_field(ui, &mut runner.params.ctx, "CONTEXTO (TOKENS)") {
                        runner.active_preset = None;
                    }
                    if num_field(ui, &mut runner.params.batch, "BATCH SIZE") {
                        runner.active_preset = None;
                    }
                    ui.label(
                        egui::RichText::new("BIND ADDRESS")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut runner.params.bind).desired_width(120.0),
                    );
                });
            });
            if runner.params.bind.trim() == "0.0.0.0" {
                ui.label(
                    egui::RichText::new(
                        "BIND 0.0.0.0 ATIVO: endpoint acessível via rede LAN/VPN \
                             sem token bearer nem TLS interno.",
                    )
                    .small()
                    .color(theme::WARN),
                );
            }
            ui.add_space(theme::SPACE_XS);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("COMANDO GERADO (CLI PREVIEW)")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let preview = runner.params.cli_preview();
                    if ui.small_button("COPIAR").clicked() {
                        ui.ctx().copy_text(preview);
                    }
                });
            });
            ui.label(
                egui::RichText::new(runner.params.cli_preview())
                    .monospace()
                    .small()
                    .color(theme::SECONDARY_FIXED),
            );
            ui.add_space(theme::SPACE_SM);
            let busy = runner.running();
            if ui
                .add_sized(
                    [ui.available_width(), 36.0],
                    egui::Button::new(
                        egui::RichText::new(if busy {
                            "○ subindo… a UI segue livre"
                        } else {
                            "SUBIR LLAMA-SERVER LOCAL"
                        })
                        .monospace()
                        .strong()
                        .color(theme::ON_PRIMARY),
                    )
                    .fill(theme::PRIMARY_CONTAINER),
                )
                .clicked()
                && !busy
            {
                runner.start();
                crate::studio_log::info(format!(
                    "runner: subida pedida ({})",
                    runner.params.cli_preview()
                ));
            }
            if !runner.error.is_empty() {
                ui.add_space(theme::SPACE_XS);
                kit::error_banner(ui, &runner.error);
            }
        });
    });
}
