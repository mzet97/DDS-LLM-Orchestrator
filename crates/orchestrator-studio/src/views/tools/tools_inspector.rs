//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_inspector(ui: &mut egui::Ui, dds: &mut DdsState) {
    let tools = &dds.snapshot.tools;
    // ── Inspetor REQUEST × RESPONSE (payloads íntegros do fio, PRD 3.13).
    // FORA: "CDR Serial", QoS Volatile/TTL e Transport (sem fonte).
    if let Some(selected_id) = dds.tools_selected.clone() {
        if let Some(tool) = tools.iter().find(|t| t.call_id == selected_id) {
            section(
                ui,
                &format!(
                    "INSPETOR DE CHAMADA SELECIONADA: {} · {}",
                    short_call_id(&tool.call_id),
                    status_label(tool.status)
                ),
            );
            egui::Frame::NONE
                .fill(theme::SURFACE_LOW)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                .inner_margin(theme::SPACE_MD)
                .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let level = crate::dds_observe::security_level_label(tool.security_level);
                    let raw_json = serde_json::json!({
                        "call_id": tool.call_id,
                        "tool_name": tool.tool_name,
                        "requester_id": tool.requester_id,
                        "security_level": level,
                        "status": status_label(tool.status),
                        "duration_ms": tool.duration_ms,
                        "arguments": serde_json::from_str::<serde_json::Value>(
                            &tool.arguments_json
                        )
                        .unwrap_or(serde_json::Value::String(tool.arguments_json.clone())),
                        "result": serde_json::from_str::<serde_json::Value>(&tool.result_json)
                            .unwrap_or(serde_json::Value::String(tool.result_json.clone())),
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "● {} · {} · {}",
                                tool.tool_name,
                                level,
                                status_label(tool.status)
                            ))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                        );
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("COPIAR RAW").monospace().small(),
                            ))
                            .clicked()
                        {
                            ui.ctx().copy_text(
                                serde_json::to_string_pretty(&raw_json).unwrap_or_default(),
                            );
                            crate::studio_log::info(format!(
                                "ferramentas: chamada {} copiada como JSON",
                                tool.call_id
                            ));
                        }
                    });
                    ui.columns(2, |cols| {
                        cols[0].label(header("PAYLOAD DE ENTRADA (arguments_json)"));
                        let args = if tool.arguments_json.is_empty() {
                            String::from("(vazio no fio)")
                        } else {
                            tool.arguments_json.clone()
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("tool-request")
                            .max_height(220.0)
                            .show(&mut cols[0], |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut args.clone())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_rows(6)
                                        .interactive(false),
                                );
                            });
                        cols[1].label(header("RESULTADO (result_json)"));
                        let result = if tool.result_json.is_empty() {
                            String::from("(ainda sem resultado)")
                        } else {
                            tool.result_json.clone()
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("tool-response")
                            .max_height(220.0)
                            .show(&mut cols[1], |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut result.clone())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_rows(6)
                                        .interactive(false),
                                );
                            });
                    });
                });
        }
    }
}
