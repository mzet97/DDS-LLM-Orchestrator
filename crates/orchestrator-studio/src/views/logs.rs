//! Painel 3.14 Logs da GUI (FIFO Ring Buffer 500): KPIs de ocupação, busca
//! por substring, filtros por nível com contagem, auto-scroll, limpeza e
//! exportação `.log` — os mesmos eventos seguem espelhados no stderr.

use crate::kit;
use crate::panel_header::panel_header;
use crate::studio_log::{self, LogsPanel};
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

    // ── Tabela: # | hora | nível (badge colorido) | evento ──
    let search = panel.search.trim().to_lowercase();
    let visible: Vec<(usize, &studio_log::LogEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            let level_ok = panel.filter_level.is_none_or(|level| entry.level == level);
            let search_ok = search.is_empty() || entry.message.to_lowercase().contains(&search);
            level_ok && search_ok
        })
        .collect();
    let scroll = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(420.0)
        .stick_to_bottom(panel.auto_scroll);
    scroll.show(ui, |ui| {
        if visible.is_empty() {
            kit::empty_state(ui, "(nenhum evento no filtro atual)");
            return;
        }
        egui::Grid::new("studio_logs").show(ui, |ui| {
            kit::grid_header(ui, &["#", "Timestamp", "Nível", "Evento / mensagem"]);
            for (index, entry) in visible {
                ui.label(
                    egui::RichText::new(index.to_string())
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                kit::mono_cell(ui, &entry.timestamp);
                ui.label(
                    egui::RichText::new(entry.level)
                        .monospace()
                        .small()
                        .color(level_color(entry.level)),
                );
                ui.label(&entry.message);
                ui.end_row();
            }
        });
    });
}
