//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_endpoint(
    ui: &mut egui::Ui,
    infer: &mut InferenceState,
    _discovery: &DiscoveryState,
) {
    // ── Endpoint HTTP + parâmetros de geração (mockup) ──
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(
                    ui,
                    "ENDPOINT DE CONTROLE HTTP & MODEL PROBE",
                    "TLS OPCIONAL · LOOPBACK/LOCAL LAN",
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("URL:")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.add_enabled_ui(!infer.busy, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut infer.server_url)
                                .desired_width(ui.available_width() - 170.0)
                                .hint_text("http://192.168.1.61:8081"),
                        );
                        if ui.button("Verificar /v1/models").clicked() {
                            infer.refresh_models();
                        }
                    });
                });
                ui.add_space(theme::SPACE_XS);
                match (infer.last_verify_ms, infer.models.is_empty()) {
                    (Some(ms), false) => {
                        ui.label(
                            egui::RichText::new(format!(
                                "● HTTP 200 OK · {ms}ms · {} modelo(s): {}",
                                infer.models.len(),
                                infer.models.join(" · ")
                            ))
                            .monospace()
                            .small()
                            .color(theme::OK),
                        );
                    }
                    _ if infer.busy && infer.reply == "listando modelos…" => {
                        ui.label(
                            egui::RichText::new("○ Verificando /v1/models…")
                                .monospace()
                                .small()
                                .color(theme::WARN),
                        );
                    }
                    _ if infer.reply.starts_with("erro") => {
                        ui.label(
                            egui::RichText::new(format!("○ {}", infer.reply))
                                .monospace()
                                .small()
                                .color(theme::ERROR),
                        );
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new(
                                "○ Sem verificação (informe a URL e clique Verificar)",
                            )
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                }
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Modelo Ativo:")
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    if infer.models.is_empty() {
                        ui.label(
                            egui::RichText::new("— (verifique /v1/models)")
                                .small()
                                .weak(),
                        );
                    } else {
                        egui::ComboBox::from_id_salt("infer-model")
                            .selected_text(if infer.model.is_empty() {
                                "—".to_owned()
                            } else {
                                infer.model.clone()
                            })
                            .show_ui(ui, |ui| {
                                for model in &infer.models {
                                    ui.selectable_value(
                                        &mut infer.model,
                                        model.clone(),
                                        model.as_str(),
                                    );
                                }
                            });
                    }
                });
            });
        });
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(
                    ui,
                    "PARÂMETROS DE GERAÇÃO (QOS T-820)",
                    temp_mode(infer.temperature),
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("TEMPERATURA  {:.2}", infer.temperature))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    card_slider(ui, &mut infer.temperature, 0.0..=2.0);
                    ui.label(
                        egui::RichText::new(temp_caption(infer.temperature))
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("MAX TOKENS  {}", infer.max_tokens))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.add(egui::DragValue::new(&mut infer.max_tokens).range(1..=8192));
                    egui::ComboBox::from_id_salt("infer-maxtok")
                        .selected_text(format!("{} tk", infer.max_tokens))
                        .show_ui(ui, |ui| {
                            for preset in [256u32, 512, 1024, 2048, 4096, 8192] {
                                ui.selectable_value(
                                    &mut infer.max_tokens,
                                    preset,
                                    format!("{preset} tk"),
                                );
                            }
                        });
                    ui.label(
                        egui::RichText::new("Limite de contexto")
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("TOP-P  {:.2}", infer.top_p))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    card_slider(ui, &mut infer.top_p, 0.0..=1.0);
                    ui.label(
                        egui::RichText::new("Nucleus sampling")
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("TIMEOUT (S)")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.label(
                        egui::RichText::new("120 seg (fixo)")
                            .monospace()
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.label(
                        egui::RichText::new("Espera segura (REQ/T-820-19)")
                            .small()
                            .color(theme::OUTLINE),
                    );
                });
            });
        });
    });
    ui.add_space(theme::SPACE_LG);
}
