//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

use super::machines_mesh::dot_of;

/// Card "Registro Manual no Catálogo" (mockup: 3 campos + Testar & Registrar).
pub(super) fn show_manual_form(ui: &mut egui::Ui, machines: &mut MachinesState) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Registro Manual no Catálogo")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("SEÇÃO 5 (PROT)")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        ui.add_space(theme::SPACE_SM);

        form_field_label(ui, "NOME DO NÓ / ALIAS", "Ex: studio-node-gpu-4090");
        let field_width = ui.available_width();
        ui.add(
            egui::TextEdit::singleline(&mut machines.form_machine_id)
                .desired_width(field_width)
                .hint_text("studio-node-delta-70"),
        );
        ui.add_space(theme::SPACE_SM);
        form_field_label(ui, "URL DO NÓ (RPC TARGET)", "http://IP:4317");
        ui.add(
            egui::TextEdit::singleline(&mut machines.form_node_url)
                .desired_width(field_width)
                .hint_text("http://192.168.1.70:4317"),
        );
        ui.add_space(theme::SPACE_SM);
        form_field_label(ui, "TOKEN / BEARER SECRET", "Guardado apenas na sessão");
        ui.horizontal(|ui| {
            let mut edit = egui::TextEdit::singleline(&mut machines.form_token)
                .hint_text("dss_sec_sk_live_…")
                .desired_width(ui.available_width() - 72.0);
            if !machines.form_token_visible {
                edit = edit.password(true);
            }
            ui.add(edit);
            if ui
                .small_button(if machines.form_token_visible {
                    "Ocultar"
                } else {
                    "Exibir"
                })
                .clicked()
            {
                machines.form_token_visible = !machines.form_token_visible;
            }
        });
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "A chave será validada por probe imediato no endpoint /version \
                 antes da persistência.",
            )
            .small()
            .weak(),
        );
        if !machines.notice.is_empty() {
            ui.label(
                egui::RichText::new(&machines.notice)
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        }
        ui.add_space(theme::SPACE_SM);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!machines.busy, |ui| {
                let register = kit::primary_button(ui, "Testar & Registrar");
                if register.clicked() {
                    machines.test_and_register();
                }
                if ui.button("Limpar").clicked() {
                    machines.clear_form();
                }
            });
        });
    });
}

/// Rótulo de campo do formulário: legenda à esquerda + dica à direita.
pub(super) fn form_field_label(ui: &mut egui::Ui, label: &str, hint: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(hint).small().color(theme::OUTLINE));
        });
    });
}

/// Card "Catálogo Compartilhado (Persistido)" (mockup: REV + tabela + rodapé).
pub(super) fn show_catalog(ui: &mut egui::Ui, machines: &mut MachinesState) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Catálogo Compartilhado (Persistido)")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!machines.busy, |ui| {
                    if ui.small_button("↻").clicked() {
                        machines.refresh();
                    }
                });
                ui.label(
                    egui::RichText::new(format!("REV: {}", machines.cursor))
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        ui.add_space(theme::SPACE_SM);

        let list = machines.machines();
        if list.is_empty() {
            kit::empty_state(
                ui,
                "Nenhuma máquina no snapshot do alvo — atualize ou registre ao lado.",
            );
        } else {
            kit::table("machines_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "Identificador Persistido",
                        "URL Base / Porta",
                        "Modo Auth",
                        "Comandos",
                    ],
                );
                for machine in &list {
                    let (dot, color) = match machines.probes.get(&machine.node_url) {
                        Some(status) => (dot_of(status.state), dot_color(status.state)),
                        None => ("●", theme::STALE),
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(dot).small().color(color));
                        kit::mono_cell(ui, &machine.machine_id);
                    });
                    kit::mono_cell(ui, &machine.node_url);
                    match machines.probes.get(&machine.node_url) {
                        None => kit::badge(ui, "SEM_RESPOSTA", theme::STALE),
                        Some(status) => match status.state {
                            ProbeState::Online => kit::badge(ui, "TOKEN_OK", theme::OK),
                            ProbeState::AuthPending => {
                                kit::badge(ui, "AUTH_PEND (401)", theme::AUTH);
                            }
                            ProbeState::Offline => kit::badge(ui, "OFFLINE", theme::ERROR),
                            ProbeState::Unknown => {
                                kit::badge(ui, "SEM_RESPOSTA", theme::STALE);
                            }
                        },
                    }
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!machines.busy, |ui| {
                            if ui.small_button("Probe").clicked() {
                                machines.probe(machine);
                            }
                            if ui.small_button("Token…").clicked() {
                                machines.modal_url = Some(machine.node_url.clone());
                                machines.modal_token.clear();
                            }
                            if ui.small_button("Remover").clicked() {
                                machines.remove_machine(&machine.machine_id);
                            }
                        });
                    });
                    ui.end_row();
                }
            });
        }
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Arquivo de Persistência: ~/.dds_studio/nodes_catalog.json")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} nós configurados", list.len()))
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
            });
        });
    });
}
