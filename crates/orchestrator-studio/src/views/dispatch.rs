//! Painel 3.6 Despacho (teste síncrono direto ao orquestrador): formulário,
//! card de resultado estruturado (agente executor · latência decomposta
//! T1–T6 · task) e histórico dos despachos da sessão com taxa de sucesso.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use crate::workload::{DispatchOutcome, DispatchState};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, dispatch: &mut DispatchState, discovery: &DiscoveryState) {
    panel_header(
        ui,
        "SEC 3.6 · DESPACHO — TASK SÍNCRONA · POST /api/v1/chat/completions/sync",
        "Despacho",
        "Task síncrona direto ao orquestrador: resposta com agente executor, \
         latência e motivo de falha tipado · pode levar minutos (agente real infere)",
    );
    // DoD PRD: chip do alvo único da descoberta (o orquestrador é o plano
    // HTTP auxiliar — endpoint próprio abaixo).
    if let Some(target) = discovery.selected_url() {
        kit::target_chip(ui, &target, "alvo da descoberta", true);
        ui.add_space(theme::SPACE_SM);
    }
    // Drena o worker de despacho (thread + mpsc — REQ/T-820-19).
    dispatch.poll();

    // ── Formulário ──
    kit::section_label(ui, "PARÂMETROS DE DESPACHO · HTTP POST DIRETO");
    ui.horizontal(|ui| {
        ui.label("endpoint do orquestrador:");
        ui.text_edit_singleline(&mut dispatch.url);
        ui.label("modelo:");
        ui.text_edit_singleline(&mut dispatch.model);
    });
    ui.label("payload de prompt / instrução técnica:");
    ui.add(
        egui::TextEdit::multiline(&mut dispatch.prompt)
            .desired_rows(3)
            .desired_width(ui.available_width())
            .hint_text("ex.: resuma o estado do nó alvo em 3 bullets"),
    );
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        let send = ui.add_enabled(
            !dispatch.busy && !dispatch.prompt.trim().is_empty(),
            egui::Button::new(
                egui::RichText::new(if dispatch.busy {
                    "◐ despachando… (agente real infere)"
                } else {
                    "▶ Despachar Task Síncrona"
                })
                .monospace()
                .color(theme::ON_PRIMARY),
            )
            .fill(theme::PRIMARY_CONTAINER),
        );
        if send.clicked() {
            dispatch.send();
        }
    });
    ui.add_space(theme::SPACE_MD);

    // ── Card do resultado estruturado (mockup 3.6) ──
    match &dispatch.last {
        Some(DispatchOutcome::Completed {
            task_id,
            assigned_agent,
            latency_ms,
            content,
            breakdown,
        }) => {
            kit::accent_card(ui, theme::OK, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("● TAREFA CONCLUÍDA").color(theme::OK));
                    kit::mono_cell(ui, task_id);
                });
                egui::Grid::new("dispatch_result").show(ui, |ui| {
                    ui.label("agente executor:");
                    ui.label(assigned_agent.as_deref().unwrap_or("?"));
                    ui.end_row();
                    ui.label("latência total:");
                    ui.label(format!("{latency_ms} ms"));
                    ui.end_row();
                });
                // Decomposição real T1–T6 do `/sync`: fila × geração ×
                // transporte × serialização (PRD 3.6).
                match breakdown {
                    Some(b) => {
                        ui.add_space(theme::SPACE_XS);
                        ui.label(
                            egui::RichText::new(format!(
                                "TOTAL {latency_ms} ms = FILA {} + GERAÇÃO {} + TRANSPORTE {} + SERIAL {}",
                                b.queue_ms, b.inference_ms, b.transport_ms, b.serial_ms
                            ))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                        );
                        // Barra proporcional honesta dos componentes.
                        let parts = [
                            (b.queue_ms, theme::STALE),
                            (b.inference_ms, theme::PRIMARY_CONTAINER),
                            (b.transport_ms, theme::WARN),
                            (b.serial_ms, theme::AUTH),
                        ];
                        let sum: u64 = parts.iter().map(|(value, _)| *value).sum::<u64>().max(1);
                        ui.horizontal(|ui| {
                            for (value, color) in parts {
                                let fraction = value as f32 / sum as f32;
                                if fraction > 0.0 {
                                    ui.add(
                                        egui::ProgressBar::new(fraction)
                                            .desired_width(fraction * 480.0)
                                            .fill(color),
                                    );
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(
                                "fila = t_agent_queue · geração = t_inference · transporte = \
                                 send+return · serial = serialize+deserialize",
                            )
                            .small()
                            .weak(),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("decomposição T1–T6 não reportada nesta resposta")
                                .small()
                                .weak(),
                        );
                    }
                }
                if let Some(content) = content {
                    ui.add_space(theme::SPACE_XS);
                    ui.label(
                        egui::RichText::new(format!(
                            "resposta: {}",
                            if content.is_empty() { "—" } else { content }
                        ))
                        .small()
                        .weak(),
                    );
                }
            });
        }
        Some(DispatchOutcome::Failed { task_id, error }) => {
            kit::accent_card(ui, theme::ERROR, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("✖ TAREFA FALHOU").color(theme::ERROR));
                    kit::mono_cell(ui, task_id);
                });
                ui.label(
                    egui::RichText::new(format!("motivo do backend: {error}"))
                        .small()
                        .color(theme::ERROR),
                );
            });
        }
        None => {
            if dispatch.busy {
                kit::badge(ui, "◐ DESPACHO EM ANDAMENTO", theme::WARN);
            } else if !dispatch.result.is_empty() {
                // erro de rede do último despacho (sem outcome estruturado)
                kit::error_banner(ui, &dispatch.result);
            } else {
                kit::empty_state(ui, "nenhum despacho nesta sessão ainda.");
            }
        }
    }
    ui.add_space(theme::SPACE_MD);

    // ── Histórico da sessão com taxa de sucesso ──
    if !dispatch.history.is_empty() {
        let ok_count = dispatch.history.iter().filter(|r| r.ok).count();
        let total = dispatch.history.len();
        let avg = {
            let latencies: Vec<u64> = dispatch
                .history
                .iter()
                .filter_map(|r| r.latency_ms)
                .collect();
            if latencies.is_empty() {
                String::from("—")
            } else {
                format!(
                    "{} ms",
                    latencies.iter().sum::<u64>() / latencies.len() as u64
                )
            }
        };
        kit::section_label(
            ui,
            &format!(
                "HISTÓRICO DA SESSÃO · TAXA DE SUCESSO {}% · LATÊNCIA MÉDIA {}",
                ok_count * 100 / total.max(1),
                avg
            ),
        );
        kit::table("dispatch_history").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Task ID",
                    "Agente executor",
                    "Latência",
                    "Fila",
                    "Geração",
                    "Desfecho",
                ],
            );
            for record in dispatch.history.iter().rev() {
                kit::mono_cell(ui, &record.task_id);
                ui.label(record.agent.as_deref().unwrap_or("—"));
                kit::num_cell(
                    ui,
                    &record
                        .latency_ms
                        .map(|ms| format!("{ms} ms"))
                        .unwrap_or_else(|| "—".to_owned()),
                );
                kit::num_cell(
                    ui,
                    &record
                        .queue_ms
                        .map(|ms| format!("{ms} ms"))
                        .unwrap_or_else(|| "—".to_owned()),
                );
                kit::num_cell(
                    ui,
                    &record
                        .inference_ms
                        .map(|ms| format!("{ms} ms"))
                        .unwrap_or_else(|| "—".to_owned()),
                );
                ui.label(if record.ok {
                    egui::RichText::new("200 OK · concluída")
                        .small()
                        .color(theme::OK)
                } else {
                    egui::RichText::new(format!("falha: {}", record.detail))
                        .small()
                        .color(theme::ERROR)
                });
                ui.end_row();
            }
        });
    }
}
