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
fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

/// Cabeçalho de card: título à esquerda + etiqueta à direita.
fn card_title(ui: &mut egui::Ui, title: &str, tag: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tag)
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
}

/// Campo numérico com parse honesto (inválido mantém o anterior).
fn num_field(ui: &mut egui::Ui, value: &mut u32, label: &str) -> bool {
    let mut raw = value.to_string();
    ui.label(
        egui::RichText::new(label)
            .monospace()
            .small()
            .color(theme::OUTLINE),
    );
    let changed = ui
        .add(egui::TextEdit::singleline(&mut raw).desired_width(90.0))
        .changed();
    if changed {
        if let Ok(parsed) = raw.trim().parse::<u32>() {
            *value = parsed;
        }
        return true;
    }
    false
}

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
                                runner.error =
                                    String::from("nenhum llama-server no PATH conhecido");
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
                            egui::TextEdit::singleline(&mut runner.params.bind)
                                .desired_width(120.0),
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

        // ── Status + etapas + console (mockup) ──
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let (status_text, status_color) = match &runner.status {
                    RunnerStatus::Stopped => (
                        String::from("○ STATUS: PARADO · HTTP HEALTH: —"),
                        theme::STALE,
                    ),
                    RunnerStatus::Starting => (
                        String::from("○ STATUS: SUBINDO · HTTP HEALTH: aguardando…"),
                        theme::WARN,
                    ),
                    RunnerStatus::Running { healthy, .. } => match healthy {
                        Some((true, ms)) => (
                            format!(
                                "● STATUS: PROCESSO ATIVO · HTTP HEALTH: 200 OK (/health · {ms}ms)"
                            ),
                            theme::OK,
                        ),
                        Some((false, _)) => (
                            String::from(
                                "● STATUS: PROCESSO ATIVO · HTTP HEALTH: FALHOU (/health)",
                            ),
                            theme::WARN,
                        ),
                        None => (
                            String::from("● STATUS: PROCESSO ATIVO · HTTP HEALTH: verificando…"),
                            theme::WARN,
                        ),
                    },
                    RunnerStatus::Stopping { .. } => (
                        String::from("○ STATUS: PARANDO · HTTP HEALTH: —"),
                        theme::WARN,
                    ),
                    RunnerStatus::Failed { detail } => {
                        (format!("× STATUS: FALHOU · {detail}"), theme::ERROR)
                    }
                };
                ui.label(
                    egui::RichText::new(status_text)
                        .monospace()
                        .small()
                        .strong()
                        .color(status_color),
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    let live = runner.pid().is_some();
                    if ui
                        .add_enabled(
                            live,
                            egui::Button::new("Parar (SIGTERM)")
                                .fill(theme::tint(theme::ERROR, 25)),
                        )
                        .clicked()
                    {
                        runner.stop();
                    }
                    if ui
                        .add_enabled(live, egui::Button::new("Reiniciar"))
                        .clicked()
                    {
                        runner.restart();
                    }
                    if ui.button("Abrir no Chat (3.3)").clicked() {
                        *go_chat = true;
                    }
                });
            });
            ui.add_space(theme::SPACE_SM);
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let done = runner.steps.iter().filter(|step| step.ok).count();
                let total_ms: u64 = runner.steps.iter().map(|step| step.duration_ms).sum();
                card_title(
                    ui,
                    "ETAPAS DE INICIALIZAÇÃO DO RUNNER",
                    &format!(
                        "{done}/{} CONCLUÍDOS (TEMPO TOTAL: {:.2}s)",
                        crate::runner::TOTAL_STEPS,
                        total_ms as f64 / 1000.0
                    ),
                );
                ui.add_space(theme::SPACE_XS);
                if runner.steps.is_empty() {
                    kit::empty_state(ui, "nenhuma execução ainda — preencha e suba.");
                }
                for step in &runner.steps {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(if step.ok { "✓" } else { "×" })
                                .strong()
                                .color(if step.ok { theme::OK } else { theme::ERROR }),
                        );
                        ui.label(
                            egui::RichText::new(step.step)
                                .small()
                                .color(theme::ON_SURFACE),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            kit::badge(
                                ui,
                                &format!("{}ms", step.duration_ms),
                                theme::PRIMARY_FIXED_DIM,
                            );
                            ui.label(
                                egui::RichText::new(&step.detail)
                                    .monospace()
                                    .small()
                                    .color(theme::ON_SURFACE_VARIANT),
                            );
                        });
                    });
                }
            });
            ui.add_space(theme::SPACE_SM);
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("LIVE OUTPUT: LLAMA-SERVER STDOUT / STDERR")
                            .monospace()
                            .size(11.5)
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    if runner.pid().is_some() {
                        kit::badge(ui, "STREAM LIVE", theme::PRIMARY_FIXED_DIM);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.toggle_value(&mut runner.autoscroll, "AUTOSCROLL: ON");
                        if ui.small_button("LIMPAR").clicked() {
                            runner.console.clear();
                        }
                    });
                });
                ui.add_space(theme::SPACE_XS);
                egui::ScrollArea::vertical()
                    .id_salt("runner-console")
                    .max_height(260.0)
                    .stick_to_bottom(runner.autoscroll)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if runner.console.is_empty() {
                            kit::empty_state(ui, "console vazia — a saída do filho aparece aqui.");
                        }
                        for line in &runner.console {
                            ui.label(
                                egui::RichText::new(line)
                                    .monospace()
                                    .small()
                                    .color(theme::ON_SURFACE_VARIANT),
                            );
                        }
                    });
                let bytes: usize = runner.console.iter().map(String::len).sum();
                ui.label(
                    egui::RichText::new(format!(
                        "TAIL: {} LINHAS · {} BUFFER",
                        runner.console.len(),
                        if bytes > 1_000_000 {
                            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
                        } else {
                            format!("{:.1} KB", bytes as f64 / 1000.0)
                        }
                    ))
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
                );
            });
            ui.add_space(theme::SPACE_XS);
            ui.label(
                egui::RichText::new(if discovery.servers.is_empty() {
                    String::from("○ SERVERSTATUS NO DOMÍNIO: nenhum anúncio")
                } else {
                    format!(
                        "● SERVERSTATUS NO DOMÍNIO: {}",
                        discovery
                            .servers
                            .iter()
                            .map(|server| server.server_id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
            );
        });
    });
}
