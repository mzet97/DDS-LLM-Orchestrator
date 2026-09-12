//! Painel subir inferência: plano legível → aplicar → prova real (§9.3, P2).

use eframe::egui;
use orchestrator_studio::launch::{Device, LaunchState};

pub fn show(ui: &mut egui::Ui, launch: &mut LaunchState, known_services: &[String]) {
    launch.poll();
    ui.heading("Subir inferência (máquina local)");
    if launch.running {
        ui.ctx().request_repaint();
    }
    ui.horizontal(|ui| {
        ui.label("Unidade do nó:");
        egui::ComboBox::from_id_salt("launch-service")
            .selected_text(if launch.plan.service.is_empty() {
                "escolha…"
            } else {
                &launch.plan.service
            })
            .show_ui(ui, |ui| {
                for name in known_services {
                    ui.selectable_value(&mut launch.plan.service, name.clone(), name);
                }
            });
        if known_services.is_empty() {
            ui.label("lista vazia — abra Serviços e clique Ler plano primeiro.");
        }
    });
    ui.horizontal(|ui| {
        ui.label("llama-server:");
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
    });
    ui.horizontal(|ui| {
        ui.label("rota DDS:");
        ui.text_edit_singleline(&mut launch.plan.dds_route);
    });
    ui.horizontal(|ui| {
        ui.label("prova:");
        ui.text_edit_singleline(&mut launch.plan.proof_prompt);
    });
    ui.collapsing("Plano (leia antes de aplicar)", |ui| {
        ui.monospace(launch.plan.preview());
    });
    if !launch.error.is_empty() {
        ui.label(&launch.error);
    }
    ui.horizontal(|ui| {
        if ui.button("Aplicar e comprovar").clicked() {
            launch.start(known_services);
        }
        if launch.running {
            ui.spinner();
            ui.label("aplicando… a UI segue livre");
        }
    });
    for step in &launch.steps {
        let mark = if step.ok { "✔" } else { "✘" };
        ui.monospace(format!("[{}] {} — {}", mark, step.step, step.detail));
    }
    if launch.proved() {
        ui.strong("Inferência comprovada com geração real.");
    }
}
