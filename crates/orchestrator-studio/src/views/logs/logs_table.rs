//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_table(
    ui: &mut egui::Ui,
    panel: &mut LogsPanel,
    live: &[LogEntry],
    toggle_pause: bool,
    do_clear: bool,
) {
    let entries: &[LogEntry] = if panel.paused { &panel.frozen } else { live };
    // ── Tabela invertida (novo primeiro, espelho do PNG) ──
    let search = panel.search.trim().to_lowercase();
    let offsets = rel_offsets(entries);
    let rows: Vec<(usize, &LogEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            let level_ok = panel.filter_level.is_none_or(|level| entry.level == level);
            let search_ok = search.is_empty()
                || entry.message.to_lowercase().contains(&search)
                || entry.source.to_lowercase().contains(&search);
            level_ok && search_ok
        })
        .collect();
    let selected_slot = panel.selected;
    let mut clicked: Option<u64> = None;
    let mut scroll = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(380.0)
        .id_salt("studio_logs_scroll");
    // Novo no topo: auto-scroll trava no offset 0 (tail invertido).
    if panel.auto_scroll && !panel.paused {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    scroll.show(ui, |ui| {
        if rows.is_empty() {
            kit::empty_state(ui, "(nenhum evento no filtro atual)");
            return;
        }
        kit::table("studio_logs").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "#L SLOT",
                    "TIMESTAMP",
                    "NÍVEL",
                    "SUBSISTEMA",
                    "CARGA ÚTIL DO EVENTO",
                    "OFFSET",
                ],
            );
            for (index, entry) in rows.iter().rev() {
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
                kit::badge(ui, entry.level, level_color(entry.level));
                kit::mono_cell(ui, &format!("[{}]", entry.source));
                ui.label(&entry.message);
                kit::num_cell(ui, &fmt_offset(offsets[*index]));
                ui.end_row();
            }
        });
    });
    if toggle_pause {
        panel.paused = !panel.paused;
        if panel.paused {
            panel.frozen = live.to_vec();
        } else {
            panel.frozen.clear();
        }
    }
    if do_clear {
        studio_log::clear();
        panel.frozen.clear();
        panel.selected = None;
    } else if let Some(slot) = clicked {
        panel.selected = (panel.selected != Some(slot)).then_some(slot);
    }
}
