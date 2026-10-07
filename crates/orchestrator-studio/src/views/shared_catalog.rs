//! Painel 3.9 Catálogo Compartilhado (multi-host, OCC): banner 409 como
//! estado de 1ª classe (base vs vigente + ação de ressincronizar), strip de
//! métricas do snapshot, gaveta de publicação com validação de JSON e base
//! condicional, filtros por kind, tabela id|kind|rev|valor e feed das
//! mutações acompanhadas. Token da autoridade (T-840-03a) só em memória.

use crate::catalog_remote::SharedCatalog;
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Kind honesto de um id: o prefixo antes de ':' (ex. `machine:orch-62`).
fn kind_of(id: &str) -> &str {
    id.split(':').next().unwrap_or("sem-prefixo")
}

/// `true` quando o aviso corrente é um conflito OCC (base obsoleta).
fn conflict_active(shared: &SharedCatalog) -> bool {
    shared.notice.contains("base obsoleta")
}

pub fn show(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    panel_header(
        ui,
        "SEC 3.9 · CATÁLOGO COMPARTILHADO — MULTI-HOST · PRIMARY LEADER NO NÓ ALVO",
        "Catálogo compartilhado",
        "Autoridade em um nó; snapshot + eventos incrementais; publicação com \
         revisão condicional (conflito 409 OCC nunca é silencioso)",
    );
    // DoD PRD: chip do alvo único (shared.url segue a seleção da descoberta).
    kit::target_chip(
        ui,
        if shared.url.is_empty() {
            "—"
        } else {
            shared.url.as_str()
        },
        if shared.busy {
            "lendo snapshot…"
        } else if shared.snapshot.is_some() {
            "snapshot carregado"
        } else {
            "sem leitura"
        },
        shared.snapshot.is_some(),
    );
    ui.add_space(theme::SPACE_SM);
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    shared.poll();

    // ── Barra da autoridade: URL + token + leituras ──
    ui.horizontal(|ui| {
        ui.label("autoridade:");
        ui.text_edit_singleline(&mut shared.url);
        ui.label("token:");
        ui.add(egui::TextEdit::singleline(&mut shared.token).password(true));
        ui.add_enabled_ui(!shared.busy, |ui| {
            if ui.button("Ler snapshot").clicked() {
                shared.refresh();
            }
            // Acompanhamento incremental desde o cursor (T-830-02).
            if ui.button("Acompanhar eventos").clicked() {
                shared.follow_events();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── Banner 409 como estado de 1ª classe (mockup 3.9) ──
    if conflict_active(shared) {
        egui::Frame::NONE
            .fill(theme::tint(theme::ERROR, 10))
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::ERROR))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new("HTTP 409 · CONFLITO DE CONCORRÊNCIA OTIMISTA (OCC)")
                        .monospace()
                        .strong()
                        .color(theme::ERROR),
                );
                ui.label(
                    egui::RichText::new(&shared.notice)
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "A base do formulário está defasada frente ao servidor — \
                         nada foi aplicado. Recarregue o snapshot e republique com a \
                         revisão vigente.",
                    )
                    .small()
                    .weak(),
                );
                ui.add_enabled_ui(!shared.busy, |ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("Atualizar Snapshot & Mesclar")
                                .monospace()
                                .small()
                                .color(theme::ON_PRIMARY),
                        ))
                        .clicked()
                    {
                        shared.refresh();
                    }
                });
            });
        ui.add_space(theme::SPACE_MD);
    } else if !shared.notice.is_empty() {
        ui.label(egui::RichText::new(&shared.notice).small().weak());
    }

    // ── Strip de métricas do snapshot ──
    let (items_len, cursor) = shared
        .snapshot
        .as_ref()
        .map(|snapshot| (snapshot.items.len(), snapshot.cursor.0))
        .unwrap_or((0, 0));
    let mut kinds: Vec<(String, usize)> = Vec::new();
    if let Some(snapshot) = &shared.snapshot {
        for item in &snapshot.items {
            let kind = kind_of(&item.id.0);
            match kinds.iter_mut().find(|(name, _)| name == kind) {
                Some((_, count)) => *count += 1,
                None => kinds.push((kind.to_owned(), 1)),
            }
        }
        kinds.sort();
    }
    let kinds_text = if kinds.is_empty() {
        String::from("—")
    } else {
        kinds
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    ui.columns(3, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Revisão do log (cursor)",
            cursor.to_string(),
            if shared.snapshot.is_some() {
                "snapshot aplicado"
            } else {
                "sem snapshot"
            },
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Registros no snapshot",
            items_len.to_string(),
            "valores íntegros da autoridade",
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Kinds mapeados",
            kinds.len().to_string(),
            &kinds_text,
            theme::ON_SURFACE_VARIANT,
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Gaveta de publicação OCC (validação de JSON + base condicional) ──
    kit::section_label(ui, "GAVETA DE PUBLICAÇÃO OCC (WRITE-LOCK CONDICIONAL)");
    egui::Grid::new("catalog_form").show(ui, |ui| {
        ui.label("id do registro:");
        ui.add(
            egui::TextEdit::singleline(&mut shared.form_id)
                .desired_width(240.0)
                .hint_text("machine:alvo-63 / cfg:chave / param:x"),
        );
        ui.label("base (r; vazia = criação):");
        ui.text_edit_singleline(&mut shared.form_base);
        ui.end_row();
        ui.label("valor (JSON):");
        ui.add(
            egui::TextEdit::multiline(&mut shared.form_value)
                .desired_rows(2)
                .desired_width(480.0)
                .hint_text(r#"{"kind":"machine","machine":{…}}"#),
        );
        ui.end_row();
    });
    let json_valid = shared.form_value.trim().is_empty()
        || serde_json::from_str::<serde_json::Value>(&shared.form_value).is_ok();
    ui.horizontal(|ui| {
        if json_valid {
            kit::badge(ui, "JSON SINTAXE VÁLIDA", theme::OK);
        } else {
            kit::badge(ui, "JSON INVÁLIDO", theme::ERROR);
        }
        ui.add_enabled_ui(!shared.busy && json_valid, |ui| {
            let publish = ui.add_enabled(
                !conflict_active(shared),
                egui::Button::new(
                    egui::RichText::new(if conflict_active(shared) {
                        "Publicar (desabilitado — ressincronize)"
                    } else {
                        "Publicar Registro"
                    })
                    .monospace()
                    .small()
                    .color(if conflict_active(shared) {
                        theme::STALE
                    } else {
                        theme::ON_PRIMARY
                    }),
                ),
            );
            if publish.clicked() {
                shared.publish_form();
            }
            if ui.button("Excluir (tombstone)").clicked() {
                shared.delete_form();
            }
        });
    });
    ui.add_space(theme::SPACE_MD);

    // ── Filtros por kind + tabela ──
    match &shared.snapshot {
        None => {
            kit::empty_state(ui, "Sem snapshot. Clique Ler snapshot.");
        }
        Some(snapshot) if snapshot.items.is_empty() => {
            kit::empty_state(ui, &format!("Catálogo vazio (cursor {cursor})."));
        }
        Some(snapshot) => {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        shared.kind_filter.is_none(),
                        egui::RichText::new(format!("Todos ({})", snapshot.items.len()))
                            .monospace()
                            .small(),
                    )
                    .clicked()
                {
                    shared.kind_filter = None;
                }
                for (kind, count) in &kinds {
                    let active = shared.kind_filter.as_deref() == Some(kind.as_str());
                    if ui
                        .selectable_label(
                            active,
                            egui::RichText::new(format!("{kind} ({count})"))
                                .monospace()
                                .small(),
                        )
                        .clicked()
                    {
                        shared.kind_filter = Some(kind.clone());
                    }
                }
            });
            ui.add_space(theme::SPACE_SM);
            egui::Grid::new("shared_catalog_grid")
                .striped(true)
                .show(ui, |ui| {
                    kit::grid_header(
                        ui,
                        &["ID do registro", "Kind", "Rev", "Valor (JSON, prévia)"],
                    );
                    for item in &snapshot.items {
                        let kind = kind_of(&item.id.0);
                        if shared
                            .kind_filter
                            .as_deref()
                            .is_some_and(|filter| filter != kind)
                        {
                            continue;
                        }
                        kit::mono_cell(ui, &item.id.0);
                        kit::mono_cell(ui, kind);
                        kit::num_cell(ui, &format!("r{}", item.revision.0));
                        let flat = item.value.replace(['\n', '\r'], " ");
                        let preview: String = flat.chars().take(64).collect();
                        let preview = if flat.chars().count() > 64 {
                            format!("{preview}…")
                        } else {
                            preview
                        };
                        ui.label(egui::RichText::new(preview).small().weak());
                        ui.end_row();
                    }
                });
        }
    }

    // ── Feed das mutações acompanhadas ──
    if !shared.feed.is_empty() {
        ui.add_space(theme::SPACE_MD);
        kit::section_label(ui, "FEED DE MUTAÇÕES INCREMENTAIS (CURSOR LOCAL)");
        egui::Frame::NONE
            .fill(theme::SURFACE_LOW)
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for entry in shared.feed.iter().rev().take(10) {
                    ui.label(
                        egui::RichText::new(format!("→ {entry}"))
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                }
            });
    }
}
