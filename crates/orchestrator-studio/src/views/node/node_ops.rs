//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

/// Card "Log de Operações Aplicadas": filtro + tabela honesta (4 colunas —
/// o contrato `OpRecord = id + op` não tem timestamp/retorno).
pub(super) fn show_ops_log(ui: &mut egui::Ui, operations: &[studio_node::operations::OpRecord]) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let mut filter: String = ui
            .ctx()
            .data(|data| data.get_temp(egui::Id::new("node-op-filter")))
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Log de Operações Aplicadas")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            kit::badge(
                ui,
                &format!("{} registradas", operations.len()),
                theme::PRIMARY_FIXED_DIM,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").clicked() {
                    filter.clear();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .desired_width(220.0)
                        .hint_text("Filtrar por id, tipo ou resumo"),
                );
            });
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new("node-op-filter"), filter.clone()));
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "Registro temporal sequencial das mutações submetidas ao daemon. Clique em \
                 uma linha para inspecionar o pacote RPC íntegro.",
            )
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
        ui.label(
            egui::RichText::new(
                "O nó não carimba horário/retorno por operação (OpRecord = id + op) — \
                 colunas de timestamp/latência não existem no contrato e ficam de fora.",
            )
            .small()
            .weak(),
        );
        ui.add_space(theme::SPACE_SM);
        let needle = filter.to_lowercase();
        let shown: Vec<&studio_node::operations::OpRecord> = operations
            .iter()
            .filter(|record| {
                needle.is_empty()
                    || record.id.0.to_lowercase().contains(&needle)
                    || op_kind(&record.op).to_lowercase().contains(&needle)
                    || op_summary(&record.op).to_lowercase().contains(&needle)
                    || op_target(&record.op).to_lowercase().contains(&needle)
            })
            .collect();
        if shown.is_empty() {
            kit::empty_state(
                ui,
                if operations.is_empty() {
                    "Nenhuma operação registrada no nó."
                } else {
                    "Nenhuma operação casa com o filtro."
                },
            );
        } else {
            let mut selected: Option<usize> = ui
                .ctx()
                .data(|data| data.get_temp(egui::Id::new("node-op-selected")));
            if selected.is_none() {
                selected = Some(0);
            }
            kit::table("node_ops_grid").show(ui, |ui| {
                kit::grid_header(ui, &["Op ID", "Tipo", "Alvo / Contexto", "Resumo da Ação"]);
                for (shown_index, record) in shown.iter().enumerate() {
                    let is_selected = selected == Some(shown_index);
                    if ui
                        .selectable_label(
                            is_selected,
                            egui::RichText::new(&record.id.0).monospace().small(),
                        )
                        .clicked()
                    {
                        selected = Some(shown_index);
                    }
                    ui.label(
                        egui::RichText::new(op_kind(&record.op))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    kit::mono_cell(ui, &op_target(&record.op));
                    ui.label(
                        egui::RichText::new(op_summary(&record.op))
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.end_row();
                }
            });
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new("node-op-selected"), selected));
        }
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "EXIBINDO {} DE {} REGISTROS",
                    shown.len(),
                    operations.len()
                ))
                .monospace()
                .small()
                .color(theme::OUTLINE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("ATUALIZADO PELA AUTO-CARGA")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
    });
}

/// Card "Inspetor de Operação RPC": JSON íntegro + encoding + tamanho real.
pub(super) fn show_inspector(ui: &mut egui::Ui, operations: &[studio_node::operations::OpRecord]) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let selected: Option<usize> = ui
            .ctx()
            .data(|data| data.get_temp(egui::Id::new("node-op-selected")));
        let filter: String = ui
            .ctx()
            .data(|data| data.get_temp(egui::Id::new("node-op-filter")))
            .unwrap_or_default();
        let needle = filter.to_lowercase();
        let shown: Vec<&studio_node::operations::OpRecord> = operations
            .iter()
            .filter(|record| {
                needle.is_empty()
                    || record.id.0.to_lowercase().contains(&needle)
                    || op_kind(&record.op).to_lowercase().contains(&needle)
                    || op_summary(&record.op).to_lowercase().contains(&needle)
                    || op_target(&record.op).to_lowercase().contains(&needle)
            })
            .collect();
        let record = selected
            .and_then(|index| shown.get(index).copied())
            .or_else(|| shown.first().copied());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Inspetor de Operação RPC")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(record) = record {
                    if ui.small_button("COPIAR").clicked() {
                        let raw = serde_json::to_string_pretty(record)
                            .unwrap_or_else(|_| format!("{record:#?}"));
                        ui.ctx().copy_text(raw);
                        crate::studio_log::info(format!(
                            "nó: operação {} copiada como JSON",
                            record.id.0
                        ));
                    }
                    kit::badge(ui, &record.id.0, theme::PRIMARY_FIXED_DIM);
                }
            });
        });
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "Estrutura de dados serializada retornada pela máquina receptora via HTTP:",
            )
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
        ui.add_space(theme::SPACE_XS);
        match record {
            Some(record) => {
                let raw_json =
                    serde_json::to_string_pretty(record).unwrap_or_else(|_| format!("{record:#?}"));
                ui.add(
                    egui::TextEdit::multiline(&mut raw_json.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(12)
                        .desired_width(ui.available_width())
                        .interactive(false),
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("TRANSPORT_ENCODING")
                                .monospace()
                                .small()
                                .color(theme::OUTLINE),
                        );
                        ui.label(
                            egui::RichText::new("application/json (UTF-8)")
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("PAYLOAD_SIZE")
                                    .monospace()
                                    .small()
                                    .color(theme::OUTLINE),
                            );
                            ui.label(
                                egui::RichText::new(format!("{} Bytes", raw_json.len()))
                                    .monospace()
                                    .small()
                                    .color(theme::ON_SURFACE_VARIANT),
                            );
                        });
                    });
                });
            }
            None => {
                kit::empty_state(ui, "Selecione uma operação na tabela ao lado.");
            }
        }
    });
}
