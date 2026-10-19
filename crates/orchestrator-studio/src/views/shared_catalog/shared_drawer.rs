//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_drawer(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    // ── Gaveta de publicação OCC (recolhível, write-lock visível) ──
    ui.horizontal(|ui| {
        ui.label(header("GAVETA DE PUBLICAÇÃO OCC"));
        if conflict_active(shared) {
            kit::badge(ui, "WRITE_LOCK: PENDING_RESYNC", theme::ERROR);
        } else {
            kit::badge(ui, "WRITE_LOCK: LIVRE", theme::OK);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(if shared.drawer_collapsed {
                    "Expandir"
                } else {
                    "Recolher"
                })
                .clicked()
            {
                shared.drawer_collapsed = !shared.drawer_collapsed;
            }
        });
    });
    if !shared.drawer_collapsed {
        ui.add_space(theme::SPACE_XS);
        // Linha ID | KIND | BASE lado a lado (espelho do PNG; o `Grid`
        // anterior colapsava as larguras dos campos — captura 3.9).
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(caption("ID DO REGISTRO"));
                ui.add(
                    egui::TextEdit::singleline(&mut shared.form_id)
                        .desired_width(420.0)
                        .hint_text("machine:alvo-63 / cfg:chave / param:x"),
                );
            });
            ui.vertical(|ui| {
                ui.label(caption("KIND / PARTIÇÃO"));
                // ComboBox real: trocar o kind reescreve o prefixo do id
                // (opções = kinds do snapshot + kind atual do formulário).
                let current_kind = kind_of(&shared.form_id).to_owned();
                let mut kinds: Vec<String> = Vec::new();
                if let Some(snapshot) = &shared.snapshot {
                    for item in &snapshot.items {
                        let kind = kind_of(&item.id.0).to_owned();
                        if !kinds.contains(&kind) {
                            kinds.push(kind);
                        }
                    }
                }
                if !kinds.contains(&current_kind) {
                    kinds.push(current_kind.clone());
                }
                kinds.sort();
                let mut picked: Option<String> = None;
                egui::ComboBox::from_id_salt("catalog_kind")
                    .selected_text(&current_kind)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for kind in &kinds {
                            if ui.selectable_label(*kind == current_kind, kind).clicked() {
                                picked = Some(kind.clone());
                            }
                        }
                    });
                if let Some(kind) = picked {
                    let rest = shared
                        .form_id
                        .split_once(':')
                        .map(|(_, rest)| rest)
                        .unwrap_or("");
                    shared.form_id = if rest.is_empty() {
                        format!("{kind}:")
                    } else {
                        format!("{kind}:{rest}")
                    };
                }
            });
            ui.vertical(|ui| {
                ui.label(caption("BASE REVISION (OCC GUARD)"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut shared.form_base)
                            .desired_width(120.0)
                            .hint_text("vazia = criação"),
                    );
                    if conflict_active(shared) {
                        kit::badge(ui, "DESATUALIZADA", theme::ERROR);
                    }
                });
            });
        });
        ui.add_space(theme::SPACE_XS);
        let json_valid = shared.form_value.trim().is_empty()
            || serde_json::from_str::<serde_json::Value>(&shared.form_value).is_ok();
        // VALOR full-width com selo JSON à direita (espelho do PNG).
        ui.horizontal(|ui| {
            ui.label(caption("VALOR JSON PAYLOAD"));
            // right_to_left: primeiro adicionado = mais à direita.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Auto-Indentar").clicked() {
                    match serde_json::from_str::<serde_json::Value>(&shared.form_value) {
                        Ok(value) => {
                            shared.form_value =
                                serde_json::to_string_pretty(&value).unwrap_or_default();
                        }
                        Err(_) => {
                            shared.notice = String::from("nada a indentar: JSON inválido");
                        }
                    }
                }
                if json_valid {
                    kit::badge(ui, "JSON SINTAXE VÁLIDA", theme::OK);
                } else {
                    kit::badge(ui, "JSON INVÁLIDO", theme::ERROR);
                }
            });
        });
        let value_width = ui.available_width();
        ui.add(
            egui::TextEdit::multiline(&mut shared.form_value)
                .desired_rows(4)
                .desired_width(value_width)
                .hint_text(r#"{"kind":"machine","machine":{…}}"#),
        );
        ui.add_space(theme::SPACE_XS);
        // Bloqueio à esquerda + ações à direita (espelho do PNG).
        ui.horizontal(|ui| {
            if let Some(info) = shared.conflict {
                ui.label(
                    egui::RichText::new(format!(
                        "Escrita bloqueada devido ao conflito HTTP 409 (base {} < vigente {}). \
                         Reincorpore o snapshot.",
                        info.attempted_base
                            .map_or(String::from("—"), |b| format!("rev.{b}")),
                        info.current
                            .map_or(String::from("—"), |c| format!("rev.{c}")),
                    ))
                    .small()
                    .color(theme::ERROR),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!shared.busy && json_valid, |ui| {
                    let blocked = conflict_active(shared);
                    let mut publish_button = egui::Button::new(
                        egui::RichText::new(if blocked {
                            "Publicar Registro (Desabilitado)"
                        } else {
                            "Publicar Registro"
                        })
                        .monospace()
                        .small()
                        .color(if blocked {
                            theme::STALE
                        } else {
                            theme::ON_PRIMARY
                        }),
                    );
                    // Habilitado = primário ciano (espelho do PNG); bloqueado
                    // = fundo padrão dimmer.
                    if !blocked {
                        publish_button = publish_button
                            .fill(theme::PRIMARY_CONTAINER)
                            .corner_radius(egui::CornerRadius::same(theme::RADIUS_PILL as u8));
                    }
                    let publish = ui.add_enabled(!blocked, publish_button);
                    if publish.clicked() {
                        shared.publish_form();
                    }
                    if ui.button("Excluir (tombstone)").clicked() {
                        shared.delete_form();
                    }
                });
            });
        });
    }
    ui.add_space(theme::SPACE_MD);
}
