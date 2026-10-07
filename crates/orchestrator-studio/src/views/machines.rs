//! Painel de Máquinas (REQ/T-840-03): registro multi-host no catálogo
//! compartilhado + sonda honesta por nó.
//!
//! O token digitado fica SÓ na memória da GUI (RNF-04 / SDD §28): nunca é
//! enviado ao catálogo, persistido ou impresso. Probe 401 (nó conforme
//! T-840-01) vira dica "token ausente" — o usuário (re)entra o token no
//! formulário e clica "Guardar token em memória".

use crate::discovery::DiscoveryState;
use crate::machines::{MachinesState, ProbeState};
use crate::theme;
use eframe::egui;

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

/// Autoridade, descoberta automática ao vivo (T-890-03), formulário de
/// registro (token = password), lista do snapshot compartilhado com ponto
/// de status e botão "Probe" por máquina.
pub fn show(ui: &mut egui::Ui, machines: &mut MachinesState, discovery: &mut DiscoveryState) {
    machines.poll();
    discovery.poll();
    ui.add_space(theme::SPACE_XS);

    // ── Hero (3.10): escuta contínua do Studio.NodePresence ──
    ui.heading("Máquinas");
    ui.label(
        egui::RichText::new(format!(
            "Escutando tráfego Studio.NodePresence · Lease 10s · Poda 30s · Heartbeat a cada 5s · Domínio {}",
            discovery.domain
        ))
        .monospace()
        .small()
        .color(theme::OUTLINE),
    );
    ui.add_space(theme::SPACE_MD);

    if discovery.scanning && discovery.nodes.is_empty() {
        ui.label(egui::RichText::new("escutando o domínio…").weak());
    }
    if !discovery.error.is_empty() {
        ui.label(egui::RichText::new(&discovery.error).color(theme::ERROR));
    }

    let now_ns = crate::machines::now_unix_ns();

    // Cards por instalação (estado do probe + idade do heartbeat + alvo).
    for (index, node) in discovery.nodes.iter().enumerate() {
        let alive = node.is_alive(now_ns);
        let age = node.age_secs(now_ns);
        let (dot_color, probe_badge): (egui::Color32, String) = match &node.probe {
            Some(probe) if probe.state == ProbeState::Online => {
                (theme::OK, format!("● {}", probe.detail))
            }
            Some(probe) if probe.state == ProbeState::AuthPending => {
                (theme::AUTH, format!("🔒 {}", probe.detail))
            }
            Some(probe) => (theme::ERROR, format!("◌ {}", probe.detail)),
            None if alive => (theme::STALE, String::from("sondando…")),
            None => (theme::STALE, String::from("—")),
        };
        let hb_label = if age <= 5 {
            "● FRESCO"
        } else if age <= 10 {
            "● VIVO"
        } else {
            "◌ EXPIRANDO"
        };
        let selected = discovery.selected == Some(index);

        egui::Frame::NONE
            .fill(if selected {
                theme::tint(theme::PRIMARY_CONTAINER, 10)
            } else {
                theme::SURFACE_CONTAINER
            })
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    // dot de presença
                    ui.painter().circle_filled(
                        egui::pos2(
                            ui.max_rect().left() + theme::SPACE_LG,
                            ui.cursor().top() + theme::SPACE_LG + 7.0,
                        ),
                        5.0,
                        if alive { theme::OK } else { theme::STALE },
                    );
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(
                                egui::RichText::new(&node.node_id)
                                    .monospace()
                                    .color(theme::ON_SURFACE),
                            );
                            ui.label(
                                egui::RichText::new(format!("[{hb_label} {age}s]"))
                                    .monospace()
                                    .small()
                                    .color(if alive { theme::OK } else { theme::STALE }),
                            );
                        });
                        ui.label(
                            egui::RichText::new(&node.url)
                                .monospace()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new(&probe_badge)
                                .monospace()
                                .small()
                                .color(dot_color),
                        );
                    });
                });
                // ação primária: selecionar como alvo (T-890-03)
                ui.add_space(theme::SPACE_XS);
                let target_label = if selected {
                    "★ ALVO DA GUI — clique para remover"
                } else {
                    "Definir como alvo da GUI"
                };
                if ui.selectable_label(selected, target_label).clicked() {
                    discovery.selected = if selected { None } else { Some(index) };
                }
            });
        ui.add_space(theme::SPACE_SM);
    }

    if discovery.nodes.is_empty() {
        ui.label(format!(
            "Nenhuma instalação vista ainda no domínio {} (heartbeats a cada 5 s; \
             nós com canvas DDS aparecem sozinhos aqui).",
            discovery.domain
        ));
    }
    ui.add_space(theme::SPACE_MD);

    // Catálogo compartilhado persistido (sempre visível, abaixo da descoberta).
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
