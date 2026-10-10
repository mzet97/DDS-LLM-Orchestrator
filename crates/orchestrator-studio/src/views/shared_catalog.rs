//! Painel 3.9 Catálogo Compartilhado (multi-host, OCC): autoridade +
//! sincronização, banner 409 estruturado (base vs vigente), 4 cards, gaveta
//! de publicação com write-lock, busca+filtros, tabela com ações e feed de
//! mutações com relógio e etiquetas.
//!
//! Só dados reais: revisões são POR CHAVE (sem "recém-atualizado" global),
//! tombstones vivem no nó (aqui só os da sessão), writer do conflito não
//! trafega (pílula sem "por ...") e o canal é HTTP follow — nada de SSE.

use crate::catalog_remote::{FeedTag, SharedCatalog};
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Kind honesto de um id: o prefixo antes de ':' (ex. `machine:orch-62`).
fn kind_of(id: &str) -> &str {
    id.split(':').next().unwrap_or("sem-prefixo")
}

/// `true` quando o aviso corrente é um conflito OCC (estruturado ou texto).
fn conflict_active(shared: &SharedCatalog) -> bool {
    shared.conflict.is_some() || shared.notice.contains("base obsoleta")
}

/// Linha visível sob kind + busca (busca casa id OU valor).
fn row_visible(id: &str, value: &str, kind: &str, filter: Option<&str>, search: &str) -> bool {
    if filter.is_some_and(|filter| filter != kind) {
        return false;
    }
    let query = search.trim();
    if query.is_empty() {
        return true;
    }
    id.contains(query) || value.contains(query)
}

pub fn show(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    panel_header(
        ui,
        "CAT-09 :: STATE_GOV",
        "3.9 Catálogo Compartilhado",
        "Catálogo Distribuído Multi-Host · Autoridade Primária no Nó Selecionado",
    );
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    shared.poll();

    // ── Autoridade + sincronização ──
    let leader = if shared.url.is_empty() {
        String::from("—")
    } else {
        kit::authority(&shared.url)
    };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("PRIMARY_LEADER: {leader}"))
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
        );
        // right_to_left: primeiro adicionado = mais à direita.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!shared.busy, |ui| {
                if ui.button("Forçar Sincronização").clicked() {
                    shared.force_sync();
                }
                if ui.button("Exportar JSON").clicked() {
                    match shared.export_snapshot() {
                        Ok(path) => {
                            shared.notice = format!("snapshot exportado: {}", path.display());
                        }
                        Err(err) => {
                            shared.notice = format!("exportação falhou: {err}");
                        }
                    }
                }
                if kit::primary_button(ui, "Publicar novo registro").clicked() {
                    shared.new_entry();
                }
                if ui.button("Ler snapshot").clicked() {
                    shared.refresh();
                }
            });
        });
    });
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        ui.label(caption("AUTORIDADE"));
        ui.add(
            egui::TextEdit::singleline(&mut shared.url)
                .desired_width(280.0)
                .hint_text("http://127.0.0.1:4317"),
        );
        ui.label(caption("TOKEN"));
        ui.add(
            egui::TextEdit::singleline(&mut shared.token)
                .password(true)
                .desired_width(160.0)
                .hint_text("só memória"),
        );
        ui.add_enabled_ui(!shared.busy, |ui| {
            if ui.button("Acompanhar eventos").clicked() {
                shared.follow_events();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── Banner 409 estruturado (base tentada × vigente do nó) ──
    if conflict_active(shared) {
        let (base_text, current_text) = match shared.conflict {
            Some(info) => (
                info.attempted_base
                    .map_or(String::from("—"), |b| format!("rev.{b}")),
                info.current
                    .map_or(String::from("—"), |c| format!("rev.{c}")),
            ),
            None => (String::from("?"), String::from("?")),
        };
        egui::Frame::NONE
            .fill(theme::tint(theme::ERROR, 10))
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::ERROR))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("HTTP 409 CONFLICT: REVISÃO CONFLITANTE")
                                    .monospace()
                                    .strong()
                                    .color(theme::ERROR),
                            );
                            ui.label(
                                egui::RichText::new(format!("Base: {base_text}"))
                                    .monospace()
                                    .small()
                                    .color(theme::ERROR),
                            );
                            ui.label(
                                egui::RichText::new(format!("Servidor: {current_text}"))
                                    .monospace()
                                    .small()
                                    .color(theme::ERROR),
                            );
                        });
                        ui.label(
                            egui::RichText::new(
                                "Conflito de concorrência otimista (OCC): o registro foi \
                                 modificado por outro escritor antes da sua publicação. \
                                 Recarregue o snapshot e republique com a revisão vigente — \
                                 nada foi aplicado.",
                            )
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        if !shared.notice.is_empty() {
                            ui.label(
                                egui::RichText::new(&shared.notice)
                                    .monospace()
                                    .small()
                                    .color(theme::WARN),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(!shared.busy, |ui| {
                            if ui.button("Atualizar Snapshot & Mesclar").clicked() {
                                shared.refresh();
                            }
                        });
                    });
                });
            });
        ui.add_space(theme::SPACE_MD);
    } else if !shared.notice.is_empty() {
        ui.label(egui::RichText::new(&shared.notice).small().weak());
        ui.add_space(theme::SPACE_SM);
    }

    // ── 4 cards do snapshot ──
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
    let last_write = shared
        .feed
        .iter()
        .rev()
        .find(|entry| matches!(entry.tag, FeedTag::Committed | FeedTag::Tombstone));
    let tomb_sub = format!("{} tombstone(s) nesta sessão", shared.session_tombstones);
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Revisão atual",
            format!("rev.{cursor}"),
            if shared.snapshot.is_some() {
                "snapshot aplicado"
            } else {
                "sem snapshot"
            },
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Total de registros",
            format!("{} itens", items_len),
            &tomb_sub,
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Tipos mapeados",
            kinds.len().to_string(),
            &kinds_text,
            theme::ON_SURFACE_VARIANT,
        );
        // O design mostra origem no card (host/WRITER), nunca o texto do
        // feed: repetir `entry.text` aqui duplicava o nó accesskit
        // (multi-match) e divergia do PNG. Writer por evento não trafega
        // no contrato — a origem honesta é a autoridade que aplicou.
        let last_write_via = format!("via {leader}");
        match last_write {
            Some(entry) => kit::metric_card(
                &mut cols[3],
                "Última gravação",
                format!("há {}", ago(entry.ts_ms)),
                &last_write_via,
                theme::WARN,
            ),
            None => kit::metric_card(
                &mut cols[3],
                "Última gravação",
                String::from("—"),
                "nenhuma gravação na sessão",
                theme::STALE,
            ),
        }
    });
    ui.add_space(theme::SPACE_MD);

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

    // ── Busca + filtros por kind + tabela com ações ──
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut shared.search)
                            .desired_width(260.0)
                            .hint_text("Filtrar por chave ou payload…"),
                    );
                });
            });
            ui.add_space(theme::SPACE_SM);
            let mut load: Option<(String, String, String)> = None;
            let mut load_tombstone = false;
            kit::table("shared_catalog_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "ID do Registro",
                        "Kind",
                        "Rev",
                        "Valor Payload (JSON)",
                        "Ações de Engenharia",
                    ],
                );
                for item in &snapshot.items {
                    let kind = kind_of(&item.id.0);
                    if !row_visible(
                        &item.id.0,
                        &item.value,
                        kind,
                        shared.kind_filter.as_deref(),
                        &shared.search,
                    ) {
                        continue;
                    }
                    kit::mono_cell(ui, &item.id.0);
                    // Chip do kind (espelho do PNG); `badge` é painter puro,
                    // sem nó accesskit — não colide com o popup do ComboBox.
                    kit::badge(ui, kind, theme::PRIMARY_FIXED_DIM);
                    kit::num_cell(ui, &format!("rev.{}", item.revision.0));
                    let flat = item.value.replace(['\n', '\r'], " ");
                    let preview: String = flat.chars().take(64).collect();
                    let preview = if flat.chars().count() > 64 {
                        format!("{preview}…")
                    } else {
                        preview
                    };
                    ui.label(egui::RichText::new(preview).small().weak())
                        .on_hover_text(&item.value);
                    ui.horizontal(|ui| {
                        if ui.button("Editar").clicked() {
                            load = Some((
                                item.id.0.clone(),
                                item.revision.0.to_string(),
                                item.value.clone(),
                            ));
                        }
                        if ui.button("Tombstone").clicked() {
                            load = Some((
                                item.id.0.clone(),
                                item.revision.0.to_string(),
                                item.value.clone(),
                            ));
                            load_tombstone = true;
                        }
                    });
                    ui.end_row();
                }
            });
            if let Some((id, base, value)) = load {
                shared.form_id = id;
                shared.form_base = base;
                shared.form_value = value;
                shared.drawer_collapsed = false;
                shared.notice = if load_tombstone {
                    String::from("registro carregado; revise e clique Excluir (tombstone)")
                } else {
                    String::from("registro carregado no formulário")
                };
            }
        }
    }

    // ── Feed de mutações (sempre visível; relógio + etiqueta por linha) ──
    ui.add_space(theme::SPACE_MD);
    ui.horizontal(|ui| {
        ui.label(header("FEED DE MUTAÇÕES INCREMENTAIS"));
        ui.label(
            egui::RichText::new("CANAL: HTTP /catalog/events (cursor)")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clear = ui.add_enabled(
                !shared.feed.is_empty(),
                egui::Button::new("Limpar visualização"),
            );
            if clear.clicked() {
                shared.feed.clear();
            }
            ui.label(
                egui::RichText::new(format!("BUFFER: {} EVTS", shared.feed.len()))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    if shared.feed.is_empty() {
        kit::empty_state(ui, "nenhum evento acompanhado nesta sessão.");
    } else {
        egui::Frame::NONE
            .fill(theme::SURFACE_LOW)
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for entry in shared.feed.iter().rev().take(10) {
                    let tag_color = match entry.tag {
                        FeedTag::Committed => theme::PRIMARY_FIXED_DIM,
                        FeedTag::Tombstone => theme::WARN,
                        FeedTag::Rejected => theme::ERROR,
                        FeedTag::Snapshot => theme::STALE,
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("[{}]", kit::clock_ms(entry.ts_ms)))
                                .monospace()
                                .small()
                                .color(theme::OUTLINE),
                        );
                        ui.label(
                            egui::RichText::new(entry.tag.label())
                                .monospace()
                                .small()
                                .strong()
                                .color(tag_color),
                        );
                        ui.label(
                            egui::RichText::new(&entry.text)
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                }
            });
    }
}

fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

/// Título de seção para fileiras (o `section_label` do kit consome a linha
/// toda e quebra `horizontal` — ver 3.6).
fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Idade curta de um instante ("14s"/"3m"/"2h") para Última gravação.
fn ago(ts_ms: u64) -> String {
    let now = now_ms();
    let secs = now.saturating_sub(ts_ms.min(now)) / 1000;
    if secs < 90 {
        format!("{secs}s")
    } else if secs < 5_400 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

fn now_ms() -> u64 {
    crate::machines::now_unix_ns() / 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_of_splits_prefix() {
        assert_eq!(kind_of("machine:orch-62"), "machine");
        assert_eq!(kind_of("sem-prefixo"), "sem-prefixo");
    }

    #[test]
    fn ago_scales_units() {
        let now = now_ms();
        assert_eq!(ago(now), "0s");
        assert_eq!(ago(now - 14_000), "14s");
        assert_eq!(ago(now - 180_000), "3m");
        assert_eq!(ago(now - 7_200_000), "2h");
        // Futuro (relógio torto) não quebra: fixa no agora.
        assert_eq!(ago(now + 60_000), "0s");
    }

    #[test]
    fn row_visible_combines_kind_and_search() {
        assert!(row_visible("machine:a", "{}", "machine", None, ""));
        assert!(!row_visible("machine:a", "{}", "machine", Some("cfg"), ""));
        assert!(row_visible(
            "machine:a",
            "{\"x\":1}",
            "machine",
            None,
            "x\":1"
        ));
        assert!(row_visible("machine:a", "{}", "machine", None, ":a"));
        assert!(!row_visible("machine:a", "{}", "machine", None, "zzz"));
        assert!(!row_visible(
            "machine:a",
            "{\"x\":1}",
            "machine",
            Some("cfg"),
            "x\":1"
        ));
    }

    #[test]
    fn conflict_covers_struct_and_legacy_text() {
        let mut shared = SharedCatalog::with_url("http://127.0.0.1:1");
        assert!(!conflict_active(&shared));
        shared.notice = String::from("base obsoleta; vigente: Some(3)");
        assert!(conflict_active(&shared));
        shared.notice.clear();
        shared.conflict = Some(crate::catalog_remote::ConflictInfo {
            attempted_base: Some(2),
            current: Some(3),
        });
        assert!(conflict_active(&shared));
    }
}
