//! Parte de `views::launch` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_status(
    cols: &mut [egui::Ui],
    runner: &mut RunnerState,
    go_chat: &mut bool,
    discovery: &DiscoveryState,
) {
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
                        String::from("● STATUS: PROCESSO ATIVO · HTTP HEALTH: FALHOU (/health)"),
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
                        egui::Button::new("Parar (SIGTERM)").fill(theme::tint(theme::ERROR, 25)),
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
}
