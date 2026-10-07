//! Painel 3.3 Inferência & Chat de Engenharia: dois planos visuais —
//! "visto no domínio" (ServerStatus DDS, cards) × "conectado para conversar"
//! (endpoint HTTP manual), parâmetros completos (temperatura/top-p/tokens) e
//! transcript com estatísticas reais por resposta (duração, tokens, t/s).

use crate::discovery::DiscoveryState;
use crate::inference::{InferenceState, Role};
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Servidor descoberto, verificação de modelos, parâmetros e transcript.
pub fn show(ui: &mut egui::Ui, infer: &mut InferenceState, discovery: &DiscoveryState) {
    panel_header(
        ui,
        &format!(
            "SEC 3.3 · INFERÊNCIA · DDS_TOPIC: STUDIO.SERVERSTATUS · DOMÍNIO {}",
            discovery.domain
        ),
        "Inferência & Chat de Engenharia",
        "Dois planos: descoberta DDS (ServerStatus — presença) × endpoint HTTP \
         (chat) · envio pode levar até 120 s (REQ/T-820-19)",
    );

    // ── Seção 1: servidores descobertos no domínio (cards) ──
    kit::section_label(ui, &format!(
        "SERVIDORES DESCOBERTOS NO DOMÍNIO (SERVERSTATUS) · {}",
        if discovery.servers.is_empty() {
            "NENHUM"
        } else {
            "AO VIVO"
        }
    ));
    if discovery.servers.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhum servidor anunciando ServerStatus no domínio {} — o \
                 llama-server precisa rodar com --enable-dds --dds-domain {} \
                 para aparecer aqui.",
                discovery.domain, discovery.domain
            ),
        );
    } else {
        for server in &discovery.servers {
            let total = server.slots_idle + server.slots_processing;
            let full = total > 0 && server.slots_idle == 0;
            let accent = if full {
                theme::ERROR
            } else if server.ready {
                theme::OK
            } else {
                theme::WARN
            };
            kit::accent_card(ui, accent, |ui| {
                ui.horizontal(|ui| {
                    kit::mono_cell(ui, &server.server_id);
                    kit::badge(
                        ui,
                        if server.ready { "PRONTO" } else { "NÃO PRONTO" },
                        accent,
                    );
                });
                ui.label(&server.model_loaded);
                ui.label(
                    egui::RichText::new(format!(
                        "KV SLOTS {}/{} ATIVOS ({} livres){}",
                        server.slots_processing,
                        total,
                        server.slots_idle,
                        if full { " · LOTADO" } else { "" }
                    ))
                    .monospace()
                    .small()
                    .color(if full {
                        theme::ERROR
                    } else {
                        theme::ON_SURFACE_VARIANT
                    }),
                );
            });
        }
        ui.label(
            egui::RichText::new("Nota: o ServerStatus NÃO carrega a URL HTTP — o \
                 endpoint do chat é manual (abaixo). Visto no domínio ≠ \
                 conectado para conversar.")
                .small()
                .weak(),
        );
    }
    ui.add_space(theme::SPACE_MD);

    // ── Seção 2: endpoint HTTP + verificação /v1/models + parâmetros ──
    kit::section_label(ui, "ENDPOINT DE CONTROLE HTTP & VERIFICAÇÃO DE MODELOS");
    ui.horizontal(|ui| {
        ui.label("servidor:");
        ui.add_enabled_ui(!infer.busy, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut infer.server_url)
                    .desired_width(260.0)
                    .hint_text("http://192.168.1.61:8081"),
            );
            if ui.button("Verificar /v1/models").clicked() {
                infer.refresh_models();
            }
        });
    });
    if !infer.reply.is_empty() {
        ui.label(egui::RichText::new(&infer.reply).small().weak());
    }
    if !infer.models.is_empty() {
        ui.label(
            egui::RichText::new(format!(
                "● HTTP OK · {} modelo(s) anunciado(s): {}",
                infer.models.len(),
                infer.models.join(" · ")
            ))
            .monospace()
            .small()
            .color(theme::OK),
        );
    }
    ui.horizontal(|ui| {
        ui.label("modelo:");
        egui::ComboBox::from_id_salt("infer-model")
            .selected_text(if infer.model.is_empty() {
                "—".to_owned()
            } else {
                infer.model.clone()
            })
            .show_ui(ui, |ui| {
                for model in &infer.models {
                    ui.selectable_value(&mut infer.model, model.clone(), model.as_str());
                }
            });
    });
    egui::Grid::new("infer_params").show(ui, |ui| {
        ui.label("temperatura:");
        ui.add(egui::Slider::new(&mut infer.temperature, 0.0..=2.0));
        ui.label("top-p:");
        ui.add(egui::Slider::new(&mut infer.top_p, 0.0..=1.0));
        ui.end_row();
        ui.label("máx. tokens:");
        ui.add(egui::DragValue::new(&mut infer.max_tokens).range(1..=8192));
        ui.label("timeout: até 120 s (REQ/T-820-19)");
        ui.end_row();
    });
    ui.separator();

    // ── Seção 3: transcript com stats reais por resposta ──
    kit::section_label(ui, "TRANSCRIPT DE ENGENHARIA & DEPURAÇÃO");
    egui::ScrollArea::vertical()
        .id_salt("infer-transcript")
        .max_height(320.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if infer.history.is_empty() {
                kit::empty_state(ui, "sessão vazia — envie o primeiro prompt abaixo.");
            }
            let mut assistant_index = 0usize;
            for message in &infer.history {
                match message.role {
                    Role::System => {
                        ui.label(egui::RichText::new(format!("sistema: {}", message.content)).weak());
                    }
                    Role::User => {
                        ui.label(
                            egui::RichText::new(format!("você: {}", message.content))
                                .color(theme::ON_SURFACE),
                        );
                    }
                    Role::Assistant => {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(format!("assistente: {}", message.content))
                                    .color(theme::SECONDARY_FIXED),
                            );
                            if let Some(stats) = infer.stats.get(assistant_index) {
                                let tokens = match (stats.prompt_tokens, stats.completion_tokens) {
                                    (Some(p), Some(c)) => format!(" · {p}+{c} tokens"),
                                    (None, Some(c)) => format!(" · {c} tokens de saída"),
                                    _ => String::new(),
                                };
                                let tps = stats
                                    .tokens_per_sec()
                                    .map(|v| format!(" · {v:.1} tok/s"))
                                    .unwrap_or_default();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "↳ {} ms{tokens}{tps}",
                                        stats.elapsed_ms
                                    ))
                                    .monospace()
                                    .small()
                                    .color(theme::OUTLINE),
                                );
                            }
                        });
                        assistant_index += 1;
                    }
                }
            }
            if infer.busy {
                ui.label(
                    egui::RichText::new("◐ gerando… (pode levar até 120 s)")
                        .monospace()
                        .small()
                        .color(theme::WARN),
                );
                ui.ctx().request_repaint();
            }
        });
    ui.add_space(theme::SPACE_SM);

    // ── Seção 4: entrada ──
    ui.horizontal(|ui| {
        ui.label("prompt:");
        let send = ui.add_enabled(
            !infer.busy && !infer.prompt.trim().is_empty(),
            egui::Button::new(
                egui::RichText::new(if infer.busy {
                    "gerando…"
                } else {
                    "Enviar Prompt"
                })
                .monospace()
                .color(theme::ON_PRIMARY),
            )
            .fill(theme::PRIMARY_CONTAINER),
        );
        if send.clicked() {
            infer.send();
        }
        if ui.button("Nova sessão").clicked() {
            infer.clear_session();
        }
    });
    ui.add(
        egui::TextEdit::multiline(&mut infer.prompt)
            .desired_rows(2)
            .desired_width(ui.available_width())
            .hint_text("digite o comando ou questão técnica…"),
    );

    // ── Rodapé: slots reais do ServerStatus ──
    if let Some(server) = discovery.servers.first() {
        let total = server.slots_idle + server.slots_processing;
        ui.add_space(theme::SPACE_SM);
        ui.label(
            egui::RichText::new(format!(
                "SLOTS {}: {} ocupado(s) · {} livre(s) de {}",
                server.server_id, server.slots_processing, server.slots_idle, total
            ))
            .monospace()
            .small()
            .color(theme::OUTLINE),
        );
    }
}
