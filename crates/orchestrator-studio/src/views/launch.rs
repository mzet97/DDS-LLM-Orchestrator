//! Painel 3.4 Subir Inferência (runner local §9.3, P2): parâmetros da
//! unidade própria do nó, CLI preview copiável, wizard nas 5 etapas
//! canônicas do PRD com duração MEDIDA, console de saídas reais das
//! operações e chip ServerStatus ao vivo (publicação ⑤).

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::launch::{Device, LaunchState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// `go_chat` vira `true` quando o operador pede "Abrir no Chat (3.3)".
pub fn show(
    ui: &mut egui::Ui,
    launch: &mut LaunchState,
    known_services: &[String],
    go_chat: &mut bool,
    discovery: &DiscoveryState,
) {
    panel_header(
        ui,
        "SEC 3.4 · SUBIR INFERÊNCIA · RUNNER LOCAL",
        "Subir inferência",
        "Sobe a unidade PRÓPRIA do nó (start de unidade existente — nada é \
         criado no host) e comprova com geração real no llama-server",
    );
    // DoD PRD: chip do alvo único (mesma URL da descoberta nas 14 telas).
    if let Some(target) = discovery.selected_url() {
        kit::target_chip(ui, &target, "alvo da descoberta", true);
        ui.add_space(theme::SPACE_SM);
    }
    launch.poll();
    if launch.running {
        ui.ctx().request_repaint();
    }

    // ── Parâmetros de inicialização ──
    kit::section_label(ui, "PARÂMETROS DE INICIALIZAÇÃO");
    ui.horizontal(|ui| {
        ui.label("Unidade do nó:");
        egui::ComboBox::from_id_salt("launch-service")
            .selected_text(if launch.plan.service.is_empty() {
                "escolha…".to_owned()
            } else {
                launch.plan.service.clone()
            })
            .show_ui(ui, |ui| {
                for name in known_services {
                    ui.selectable_value(&mut launch.plan.service, name.clone(), name);
                }
            });
        if known_services.is_empty() {
            ui.label(
                egui::RichText::new("lista vazia — abra Serviços e clique Ler plano primeiro.")
                    .small()
                    .color(theme::WARN),
            );
        }
    });
    ui.horizontal(|ui| {
        ui.label("llama-server (HTTP):");
        ui.text_edit_singleline(&mut launch.plan.llama_url);
        ui.label("dispositivo:");
        ui.selectable_value(&mut launch.plan.device, Device::Gpu, "GPU");
        ui.selectable_value(&mut launch.plan.device, Device::Cpu, "CPU");
    });
    ui.horizontal(|ui| {
        let mut ctx = launch.plan.ctx_tokens.to_string();
        ui.label("contexto:");
        if ui.text_edit_singleline(&mut ctx).changed() {
            launch.plan.ctx_tokens = ctx.parse().unwrap_or(launch.plan.ctx_tokens);
        }
        let mut slots = launch.plan.slots.to_string();
        ui.label("slots:");
        if ui.text_edit_singleline(&mut slots).changed() {
            launch.plan.slots = slots.parse().unwrap_or(launch.plan.slots);
        }
        ui.label("rota DDS:");
        ui.text_edit_singleline(&mut launch.plan.dds_route);
    });
    ui.horizontal(|ui| {
        ui.label("prompt da prova:");
        ui.text_edit_singleline(&mut launch.plan.proof_prompt);
    });
    ui.add_space(theme::SPACE_SM);

    // ── CLI preview (copiável) ──
    kit::section_label(ui, "PLANO — LEIA ANTES DE APLICAR (CLI PREVIEW)");
    let preview = launch.plan.preview();
    egui::Frame::NONE
        .fill(theme::SURFACE_LOW)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if ui.button("Copiar").clicked() {
                    ui.ctx().copy_text(preview.clone());
                    crate::studio_log::info("subir: CLI preview copiado");
                }
                ui.label(
                    egui::RichText::new("NADA é criado no nó: só start de unidade própria.")
                        .small()
                        .color(theme::WARN),
                );
            });
            ui.label(egui::RichText::new(&preview).monospace().small());
        });
    ui.add_space(theme::SPACE_MD);

    if !launch.error.is_empty() {
        kit::error_banner(ui, &launch.error);
    }

    // ── Ação + atalho para o chat ──
    ui.horizontal(|ui| {
        let apply = ui.add_enabled(
            !launch.running && !launch.plan.service.is_empty(),
            egui::Button::new(
                egui::RichText::new(if launch.running {
                    "◐ aplicando… a UI segue livre"
                } else {
                    "Aplicar e Comprovar"
                })
                .monospace()
                .color(theme::ON_PRIMARY),
            )
            .fill(theme::PRIMARY_CONTAINER),
        );
        if apply.clicked() {
            launch.start(known_services);
        }
        if ui.button("Abrir no Chat (3.3)").clicked() {
            *go_chat = true;
        }
    });
    ui.add_space(theme::SPACE_MD);

    // ── Wizard de etapas com duração medida ──
    if !launch.steps.is_empty() {
        let done_count = launch.steps.iter().filter(|step| step.ok).count();
        let total_ms: u64 = launch.steps.iter().map(|step| step.duration_ms).sum();
        kit::section_label(
            ui,
            &format!(
                "ETAPAS DE INICIALIZAÇÃO · {}/{} CONCLUÍDAS · {} ms",
                done_count,
                launch.steps.len(),
                total_ms
            ),
        );
        for step in &launch.steps {
            let (mark, color) = if step.ok {
                ("✔", theme::OK)
            } else {
                ("✘", theme::ERROR)
            };
            kit::accent_card(ui, color, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(mark).monospace().strong().color(color));
                    ui.strong(step.step);
                    ui.label(
                        egui::RichText::new(format!("· {} ms", step.duration_ms))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                });
                ui.label(egui::RichText::new(&step.detail).small().weak());
            });
        }
        if launch.proved() {
            ui.label(
                egui::RichText::new("● INFERÊNCIA COMPROVADA COM GERAÇÃO REAL")
                    .monospace()
                    .strong()
                    .color(theme::OK),
            );
        }

        // ── Console de saídas reais (PRD 3.4): o que o nó/servidor ──
        //    responderiu a cada etapa — stdout/stderr do subprocesso não
        //    atravessam a API do nó; aqui vão as respostas literais.
        ui.add_space(theme::SPACE_SM);
        kit::section_label(
            ui,
            "CONSOLE · SAÍDAS REAIS DAS OPERAÇÕES (RESPOSTAS DO NÓ/SERVIDOR)",
        );
        egui::Frame::NONE
            .fill(theme::SURFACE_LOW)
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for step in &launch.steps {
                    let prefix = if step.ok { "OUT" } else { "ERR" };
                    let color = if step.ok {
                        theme::ON_SURFACE_VARIANT
                    } else {
                        theme::ERROR
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "[{prefix} · {} ms] {} → {}",
                            step.duration_ms, step.step, step.detail
                        ))
                        .monospace()
                        .small()
                        .color(color),
                    );
                }
                ui.label(
                    egui::RichText::new(
                        "stdout/stderr do subprocesso não atravessam a API do nó — \
                         as linhas acima são as respostas literais das operações.",
                    )
                    .small()
                    .weak(),
                );
            });

        // ⑤ publicação ServerStatus ao vivo (sinal de convergência DDS).
        ui.add_space(theme::SPACE_XS);
        if discovery.servers.is_empty() {
            kit::badge(ui, "⑤ SERVERSTATUS: NENHUM NO DOMÍNIO AINDA", theme::WARN);
        } else {
            let servers: Vec<&str> = discovery
                .servers
                .iter()
                .map(|server| server.server_id.as_str())
                .collect();
            kit::badge(
                ui,
                &format!("⑤ SERVERSTATUS NO DOMÍNIO: {}", servers.join(" · ")),
                theme::OK,
            );
        }
    } else if !launch.running {
        kit::empty_state(ui, "nenhuma execução ainda — escolha a unidade e aplique.");
    }
}
