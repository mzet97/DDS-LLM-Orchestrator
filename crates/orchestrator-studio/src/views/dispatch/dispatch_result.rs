//! Parte de `views::dispatch` (extraída para manter os arquivos revisáveis).

/// Coluna direita: card do resultado estruturado.
use super::*;

use crate::workload::DispatchOutcome;

pub(super) fn result_column(ui: &mut egui::Ui, dispatch: &mut DispatchState) {
    // Aba em local (o `match` abaixo toma `&dispatch.last` emprestado).
    let mut tab = dispatch.selected_tab.min(2);
    match &dispatch.last {
        Some(DispatchOutcome::Completed {
            task_id,
            assigned_agent,
            latency_ms,
            content,
            breakdown,
            http_status,
            raw,
            headers,
        }) => {
            let status_text = if *http_status == 200 {
                String::from("200 OK")
            } else {
                http_status.to_string()
            };
            kit::accent_card(ui, theme::OK, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("● TAREFA CONCLUÍDA (Status: {status_text})"))
                            .color(theme::OK)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        kit::mono_cell(ui, &format!("ID: {task_id}"));
                    });
                });
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(caption("AGENTE EXECUTOR"));
                        ui.label(
                            egui::RichText::new(
                                assigned_agent.as_deref().unwrap_or("não informado"),
                            )
                            .strong()
                            .color(theme::PRIMARY_FIXED_DIM),
                        );
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(caption("LATÊNCIA TOTAL"));
                        ui.label(egui::RichText::new(format!("{latency_ms} ms")).strong());
                        if let Some(b) = breakdown {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Fila: {} ms | Inf: {} ms",
                                    b.queue_ms, b.inference_ms
                                ))
                                .small()
                                .weak(),
                            );
                        }
                    });
                });
                // Decomposição real T1–T6 do `/sync`: fila × geração ×
                // transporte × serialização (âncora kittest_ux2 — formato
                // exato preservado).
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
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut tab, 0, "Resposta Formatada");
                    ui.selectable_value(&mut tab, 1, "Payload JSON Bruto");
                    ui.selectable_value(&mut tab, 2, "Headers & Telemetria");
                });
                let body = match tab {
                    0 => content
                        .as_deref()
                        .filter(|text| !text.is_empty())
                        .unwrap_or("— (backend não retornou conteúdo)")
                        .to_owned(),
                    1 => raw.clone(),
                    _ => format!("HTTP {http_status}\n{headers}"),
                };
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Copiar").clicked() {
                            ui.ctx().copy_text(body.clone());
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&body).monospace().small());
                    });
            });
        }
        Some(DispatchOutcome::Failed { task_id, error }) => {
            kit::accent_card(ui, theme::ERROR, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("× TAREFA FALHOU").color(theme::ERROR));
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
            // Cabeçalho próprio: equilibra a coluna com os parâmetros.
            ui.label(header("○ RESULTADO DO DESPACHO"));
            ui.add_space(theme::SPACE_XS);
            if dispatch.busy {
                kit::badge(ui, "○ DESPACHO EM ANDAMENTO", theme::WARN);
            } else {
                kit::empty_state(ui, "nenhum despacho nesta sessão ainda.");
            }
        }
    }
    dispatch.selected_tab = tab;
}
