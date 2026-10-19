//! Parte de `views::workflow` (extraída para manter os arquivos revisáveis).

/// Faixa de 4 métricas do pipeline (espelho do PNG; 4º card honesto —
/// %LOSS/heartbeats/NACKs não têm fonte, então vale "Último erro").
#[cfg(feature = "dds")]
use super::*;

use super::workflow_audit::section;
use super::workflow_stage::{fmt_int, stage_display, stage_of, stage_visual, StageVisual};

pub(super) fn metrics(ui: &mut egui::Ui, state: &mut WorkflowState) {
    ui.add_space(theme::SPACE_MD);
    let lat = |name: &str| {
        stage_of(&state.stages, name)
            .map(|stage| fmt_int(stage.latency_ms))
            .unwrap_or_else(|| "—".to_owned())
    };
    let breakdown = format!(
        "Soma de Estágios A ({}) + B ({}) + C ({})",
        lat("A"),
        lat("B"),
        lat("C")
    );
    // Valores HERO 42px: curtos (achado 3.7 — longo quebra em 3 linhas).
    let (state_value, state_sub, state_accent) = if state.busy {
        ("Em execução", "estágios em andamento", theme::WARN)
    } else if state.error.is_some() {
        ("Falhou", "ver exceções abaixo", theme::ERROR)
    } else if state.total_ms.is_some() {
        ("Concluído", "sem retry silencioso", theme::OK)
    } else {
        ("Aguardando", "nenhuma execução ainda", theme::STALE)
    };
    let delivered = state.stages.len();
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Tempo total cumulativo",
            state
                .total_ms
                .map(|ms| format!("{} ms", fmt_int(ms)))
                .unwrap_or_else(|| "—".to_owned()),
            &breakdown,
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Estado do pipeline",
            state_value.to_owned(),
            state_sub,
            state_accent,
        );
        kit::metric_card(
            &mut cols[2],
            "Tópico de saída final",
            String::from("TaskOutput"),
            &format!("Tasks → TaskOutput · {delivered}/3 entregues"),
            if delivered == 3 {
                theme::OK
            } else if delivered > 0 {
                theme::WARN
            } else {
                theme::STALE
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Último erro",
            if state.error.is_some() {
                "sim".to_owned()
            } else {
                "nenhum".to_owned()
            },
            if state.busy {
                "executando…"
            } else {
                "ver exceções abaixo"
            },
            if state.error.is_some() {
                theme::ERROR
            } else {
                theme::OK
            },
        );
    });
    ui.add_space(theme::SPACE_MD);
}

/// Cadeia distribuída: 3 cards de estágio com conectores (espelho do PNG).
#[cfg(feature = "dds")]
pub(super) fn stage_cards(ui: &mut egui::Ui, state: &mut WorkflowState) {
    section(ui, "CADEIA DISTRIBUÍDA EM EXECUÇÃO (A → B → C)");
    let visuals: [(String, StageVisual); 3] = [
        (String::from("A"), stage_visual(state, "A")),
        (String::from("B"), stage_visual(state, "B")),
        (String::from("C"), stage_visual(state, "C")),
    ];
    // Conectores A→B→C com o estágio de origem concluído destacado.
    let connector = |from: StageVisual| -> (egui::Color32, &'static str) {
        match from {
            StageVisual::Concluido => (theme::OK, "●——▶"),
            StageVisual::Falhou => (theme::ERROR, "●——×"),
            _ => (theme::STALE, "○——▶"),
        }
    };
    // `accent_card` força largura total: cada card vai num `vertical` de
    // largura fixa (sem isso só o A aparece — captura 3.12).
    let card_w = ((ui.available_width() - 2.0 * 56.0 - 4.0 * 8.0) / 3.0).max(230.0);
    ui.horizontal(|ui| {
        for (index, (name, visual)) in visuals.iter().enumerate() {
            if index > 0 {
                let (_, previous) = visuals[index - 1].clone();
                let (color, symbol) = connector(previous);
                ui.vertical(|ui| {
                    ui.set_min_width(56.0);
                    ui.set_max_width(56.0);
                    ui.add_space(48.0);
                    ui.label(
                        egui::RichText::new(symbol)
                            .monospace()
                            .color(color)
                            .size(16.0),
                    );
                });
            }
            let (accent, state_label) = match visual {
                StageVisual::Concluido => (theme::OK, "● CONCLUÍDO"),
                StageVisual::Executando => (theme::WARN, "○ EXECUTANDO"),
                StageVisual::Falhou => (theme::ERROR, "× FALHOU"),
                StageVisual::Pendente => (theme::STALE, "○ PENDENTE"),
            };
            ui.vertical(|ui| {
                ui.set_min_width(card_w);
                ui.set_max_width(card_w);
                kit::accent_card(ui, accent, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(stage_display(name)).strong().size(14.0));
                        kit::badge(ui, state_label, accent);
                    });
                    match stage_of(&state.stages, name) {
                        Some(stage) => {
                            // task_id real é UUID v4 (36 chars): 8 à mostra,
                            // íntegra no hover. FORA: "Executor DDS" (o `TaskResult`
                            // não carrega executor) e latência "QoS SUB" (sem fonte).
                            ui.horizontal(|ui| {
                                ui.label(caption("TASK ID:"));
                                ui.label(
                                    egui::RichText::new(
                                        stage.task_id.chars().take(8).collect::<String>(),
                                    )
                                    .monospace()
                                    .small()
                                    .color(theme::PRIMARY_FIXED_DIM),
                                )
                                .on_hover_text(&stage.task_id);
                            });
                            ui.horizontal(|ui| {
                                ui.label(caption("LATÊNCIA:"));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ms",
                                        fmt_int(stage.latency_ms)
                                    ))
                                    .monospace()
                                    .small()
                                    .color(theme::PRIMARY_FIXED_DIM),
                                );
                            });
                            ui.label(caption("PRÉVIA DO BUFFER (96 CHARS):"));
                            egui::Frame::NONE
                                .fill(theme::SURFACE_LOW)
                                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                                .inner_margin(theme::SPACE_SM)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(
                                        egui::RichText::new(if stage.preview.is_empty() {
                                            "—"
                                        } else {
                                            &stage.preview
                                        })
                                        .small()
                                        .weak(),
                                    );
                                });
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("DDS PUB: Tasks")
                                        .monospace()
                                        .small()
                                        .color(theme::ON_SURFACE_VARIANT),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("OK 100%")
                                                .monospace()
                                                .small()
                                                .strong()
                                                .color(theme::PRIMARY_FIXED_DIM),
                                        );
                                    },
                                );
                            });
                        }
                        None => {
                            ui.label(
                                egui::RichText::new("aguardando o estágio anterior…")
                                    .small()
                                    .weak(),
                            );
                        }
                    }
                });
            });
        }
    });
    ui.add_space(theme::SPACE_MD);
}
