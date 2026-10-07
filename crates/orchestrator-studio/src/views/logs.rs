//! Painel 3.14 Logs da GUI (FIFO Ring Buffer 500): KPIs de ocupação, busca
//! por substring, filtros por nível com contagem, auto-scroll, limpeza e
//! exportação `.log` — os mesmos eventos seguem espelhados no stderr.
//!
//! PRD 3.14: colunas SUBSISTEMA e OFFSET (Δ ms) + inspetor lateral da
//! entrada selecionada (SLOT # · THREAD · FONTE file:line · mensagem crua
//! · STACK TRACE em erros · CONTEXTO payload JSON).

use crate::kit;
use crate::panel_header::panel_header;
use crate::studio_log::{self, LogEntry, LogsPanel};
use crate::theme;
use eframe::egui;

/// Níveis fixos do buffer (mesma ordem do enum de push).
const LEVELS: [&str; 3] = ["INFO", "WARN", "ERRO"];

fn level_color(level: &str) -> egui::Color32 {
    match level {
        "ERRO" => theme::ERROR,
        "WARN" => theme::WARN,
        _ => theme::OK,
    }
}

pub fn show(ui: &mut egui::Ui, panel: &mut LogsPanel) {
    panel_header(
        ui,
        "SEC 3.14 · LOGS DA GUI — FIFO RING BUFFER 500",
        "Logs",
        "Eventos internos da GUI (descoberta, alvo, auto-carga, observação) \
         espelhados no stderr · FIFO: eventos antigos são descartados",
    );
    let entries = studio_log::entries();
    let info_count = entries.iter().filter(|e| e.level == "INFO").count();
    let warn_count = entries.iter().filter(|e| e.level == "WARN").count();
    let error_count = entries.iter().filter(|e| e.level == "ERRO").count();

    // ── KPIs (mockup 3.14): ocupação · severidade · sink stderr ──
    ui.columns(3, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Ocupação do ring buffer",
            format!("{}/500", entries.len()),
            if entries.len() >= 500 {
                "FIFO evictando eventos antigos"
            } else {
                "com espaço"
            },
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Severidade",
            format!("{error_count} erro(s) ativos"),
            &format!("{info_count} INFO · {warn_count} WARN"),
            if error_count > 0 {
                theme::ERROR
            } else if warn_count > 0 {
                theme::WARN
            } else {
                theme::OK
            },
        );
        kit::metric_card(
            &mut cols[2],
            "Sink stderr",
            "SINC".to_owned(),
            "mesmos eventos em /dev/stderr [fd 2] · sem drop",
            theme::OK,
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Toolbar: filtros com contagem + busca + ações ──
    ui.horizontal(|ui| {
        if ui
            .selectable_label(
                panel.filter_level.is_none(),
                egui::RichText::new(format!("Todos ({})", entries.len()))
                    .monospace()
                    .small(),
            )
            .clicked()
        {
            panel.filter_level = None;
        }
        for level in LEVELS {
            let count = entries.iter().filter(|e| e.level == level).count();
            if ui
                .selectable_label(
                    panel.filter_level == Some(level),
                    egui::RichText::new(format!("{level} ({count})"))
                        .monospace()
                        .small()
                        .color(level_color(level)),
                )
                .clicked()
            {
                panel.filter_level = Some(level);
            }
        }
        ui.separator();
        ui.checkbox(&mut panel.auto_scroll, "Auto-scroll");
        if ui.button("Limpar buffer").clicked() {
            studio_log::clear();
            panel.selected = None;
        }
        if ui.button("Exportar (.log)").clicked() {
            match studio_log::export() {
                Ok(path) => studio_log::info(format!("logs exportados em {}", path.display())),
                Err(err) => studio_log::error(format!("falha ao exportar logs: {err}")),
            }
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut panel.search)
            .hint_text("filtro rápido (ex: ECONNREFUSED, lease, 409, orchestrator, .62)…")
            .desired_width(420.0)
            .font(egui::TextStyle::Monospace),
    );
    ui.add_space(theme::SPACE_SM);

    // ── Tabela: # | hora | nível | subsistema | evento | offset ──
    let search = panel.search.trim().to_lowercase();
    let visible: Vec<&LogEntry> = entries
        .iter()
        .filter(|entry| {
            let level_ok = panel.filter_level.is_none_or(|level| entry.level == level);
            let search_ok = search.is_empty()
                || entry.message.to_lowercase().contains(&search)
                || entry.source.to_lowercase().contains(&search);
            level_ok && search_ok
        })
        .collect();
    let selected_slot = panel.selected;
    let mut clicked: Option<u64> = None;
    let scroll = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(380.0)
        .stick_to_bottom(panel.auto_scroll);
    scroll.show(ui, |ui| {
        if visible.is_empty() {
            kit::empty_state(ui, "(nenhum evento no filtro atual)");
            return;
        }
        egui::Grid::new("studio_logs").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "SLOT #",
                    "Timestamp",
                    "Nível",
                    "Subsistema",
                    "Evento / mensagem",
                    "Offset",
                ],
            );
            for entry in &visible {
                let is_selected = selected_slot == Some(entry.slot);
                if ui
                    .selectable_label(
                        is_selected,
                        egui::RichText::new(entry.slot.to_string())
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    )
                    .clicked()
                {
                    clicked = Some(entry.slot);
                }
                kit::mono_cell(ui, &entry.timestamp);
                ui.label(
                    egui::RichText::new(entry.level)
                        .monospace()
                        .small()
                        .color(level_color(entry.level)),
                );
                kit::mono_cell(ui, entry.source);
                ui.label(&entry.message);
                kit::num_cell(ui, &format!("+{} ms", entry.offset_ms));
                ui.end_row();
            }
        });
    });
    if let Some(slot) = clicked {
        panel.selected = (panel.selected != Some(slot)).then_some(slot);
    }

    // ── Inspetor lateral (PRD 3.14): detalhes forenses da entrada ──
    if let Some(slot) = panel.selected {
        if let Some(entry) = entries.iter().find(|e| e.slot == slot) {
            ui.add_space(theme::SPACE_SM);
            kit::accent_card(ui, level_color(entry.level), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!("INSPETOR · SLOT #{}", entry.slot))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("fechar").monospace().small(),
                        ))
                        .clicked()
                    {
                        panel.selected = None;
                    }
                });
                egui::Grid::new("log_inspector").show(ui, |ui| {
                    ui.label("thread:");
                    kit::mono_cell(ui, &entry.thread);
                    ui.end_row();
                    ui.label("fonte:");
                    kit::mono_cell(ui, &entry.origin);
                    ui.end_row();
                    ui.label("subsistema:");
                    kit::mono_cell(ui, entry.source);
                    ui.end_row();
                    ui.label("momento:");
                    kit::mono_cell(
                        ui,
                        &format!("{} · +{} ms do anterior", entry.timestamp, entry.offset_ms),
                    );
                    ui.end_row();
                });
                ui.label(
                    egui::RichText::new("MENSAGEM CRUA")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut entry.message.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(1)
                        .interactive(false),
                );
                if let Some(payload) = &entry.payload {
                    ui.label(
                        egui::RichText::new("CONTEXTO (payload serializado)")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut payload.clone())
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(2)
                            .interactive(false),
                    );
                }
                if let Some(backtrace) = &entry.backtrace {
                    ui.label(
                        egui::RichText::new("STACK TRACE (Rust · capturado no registro)")
                            .monospace()
                            .small()
                            .color(theme::ERROR),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut backtrace.clone())
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(6)
                            .interactive(false),
                    );
                }
            });
        }
    }
}
