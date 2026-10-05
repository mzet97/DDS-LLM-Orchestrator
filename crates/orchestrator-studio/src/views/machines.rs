//! Painel de Máquinas (REQ/T-840-03): registro multi-host no catálogo
//! compartilhado + sonda honesta por nó.
//!
//! O token digitado fica SÓ na memória da GUI (RNF-04 / SDD §28): nunca é
//! enviado ao catálogo, persistido ou impresso. Probe 401 (nó conforme
//! T-840-01) vira dica "token ausente" — o usuário (re)entra o token no
//! formulário e clica "Guardar token em memória".

use eframe::egui;
use orchestrator_studio::machines::{MachinesState, ProbeState};

/// Cor/símbolo do ponto de status por estado da sonda.
fn dot_style(state: ProbeState) -> (egui::Color32, &'static str) {
    match state {
        ProbeState::Online => (egui::Color32::from_rgb(48, 209, 88), state.dot()),
        ProbeState::Offline => (egui::Color32::from_rgb(255, 105, 97), state.dot()),
        ProbeState::AuthPending | ProbeState::Unknown => {
            (egui::Color32::from_rgb(255, 214, 10), state.dot())
        }
    }
}

/// Autoridade, formulário de registro (token = password), lista do snapshot
/// compartilhado com ponto de status e botão "Probe" por máquina.
pub fn show(ui: &mut egui::Ui, machines: &mut MachinesState) {
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    machines.poll();
    ui.collapsing("Máquinas (catálogo compartilhado)", |ui| {
        ui.horizontal(|ui| {
            ui.label("autoridade:");
            ui.text_edit_singleline(&mut machines.url);
            ui.label("token da autoridade:");
            ui.add(egui::TextEdit::singleline(&mut machines.authority_token).password(true));
            ui.add_enabled_ui(!machines.busy, |ui| {
                if ui.button("Ler snapshot").clicked() {
                    machines.refresh();
                }
            });
        });
        ui.separator();
        ui.label("Registro (token fica só na memória local — nunca vai ao catálogo):");
        egui::Grid::new("machines_form_grid").show(ui, |ui| {
            ui.label("machine_id:");
            ui.text_edit_singleline(&mut machines.form_machine_id);
            ui.label("host:");
            ui.text_edit_singleline(&mut machines.form_host);
            ui.end_row();
            ui.label("user:");
            ui.text_edit_singleline(&mut machines.form_user);
            ui.label("node_url:");
            ui.text_edit_singleline(&mut machines.form_node_url);
            ui.end_row();
            ui.label("token:");
            ui.add(egui::TextEdit::singleline(&mut machines.form_token).password(true));
            ui.label("services_hint (csv):");
            ui.text_edit_singleline(&mut machines.form_services_hint);
            ui.end_row();
            ui.label("base:");
            ui.text_edit_singleline(&mut machines.form_base);
            ui.label("(vazia = criação; 409 preenche com a vigente)");
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!machines.busy, |ui| {
                if ui.button("Publicar no catálogo").clicked() {
                    let _ = machines.publish_machine();
                }
                if ui.button("Guardar token em memória").clicked() {
                    machines.save_token_from_form();
                }
            });
        });
        if !machines.notice.is_empty() {
            ui.label(&machines.notice);
        }
        if !machines.missing_token_urls().is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(255, 214, 10),
                "token ausente: preencha o campo token (com o node_url da máquina) \
                 e clique \u{201c}Guardar token em memória\u{201d}",
            );
        }
        ui.separator();
        let list = machines.machines();
        if list.is_empty() {
            ui.label("Nenhuma máquina no snapshot. Clique Ler snapshot ou registre acima.");
        } else {
            egui::Grid::new("machines_grid").show(ui, |ui| {
                ui.label("machine_id");
                ui.label("host");
                ui.label("node_url");
                ui.label("status");
                ui.label("detalhe");
                ui.label("");
                ui.end_row();
                for machine in &list {
                    ui.label(&machine.machine_id);
                    ui.label(&machine.host);
                    ui.label(&machine.node_url);
                    match machines.probes.get(&machine.node_url) {
                        None => {
                            ui.colored_label(egui::Color32::GRAY, ProbeState::Unknown.dot());
                            ui.label("não sondado");
                        }
                        Some(status) => {
                            let (color, dot) = dot_style(status.state);
                            ui.colored_label(color, dot);
                            ui.label(&status.detail);
                        }
                    }
                    ui.add_enabled_ui(!machines.busy, |ui| {
                        if ui.button("Probe").clicked() {
                            machines.probe(machine);
                        }
                    });
                    ui.end_row();
                }
            });
        }
    });
}
