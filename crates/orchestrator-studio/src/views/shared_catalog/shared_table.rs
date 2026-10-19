//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_table(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    let cursor = shared
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.cursor.0)
        .unwrap_or(0);
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
}
