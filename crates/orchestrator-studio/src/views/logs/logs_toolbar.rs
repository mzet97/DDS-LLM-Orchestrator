//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_toolbar(
    ui: &mut egui::Ui,
    panel: &mut LogsPanel,
    live: &[LogEntry],
) -> (bool, bool) {
    // Lista exibida: congelada sob pausa (slide do buffer segue vivo).
    let entries: &[LogEntry] = if panel.paused { &panel.frozen } else { live };
    // Ações comutadas na toolbar, aplicadas após o bloco de leitura.
    let mut toggle_pause = false;
    let mut do_clear = false;
    // ── Toolbar: filtros + busca + toggles + ações ──
    ui.horizontal_wrapped(|ui| {
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
        ui.add(
            egui::TextEdit::singleline(&mut panel.search)
                .hint_text("filtro rápido (substring)…")
                .desired_width(240.0)
                .font(egui::TextStyle::Monospace),
        );
        ui.separator();
        ui.checkbox(&mut panel.auto_scroll, "Auto-scroll");
        if ui.checkbox(&mut panel.mirror, "Espelhar stderr").changed() {
            studio_log::set_mirror(panel.mirror);
        }
        if ui
            .button(if panel.paused { "Retomar" } else { "Pausar" })
            .clicked()
        {
            toggle_pause = true;
        }
        if ui.button("Limpar buffer").clicked() {
            do_clear = true;
        }
        if ui.button("Exportar (.log)").clicked() {
            match studio_log::export() {
                Ok(path) => {
                    studio_log::info(format!("logs exportados em {}", path.display()));
                }
                Err(err) => studio_log::error(format!("falha ao exportar logs: {err}")),
            }
        }
        if panel.paused {
            kit::badge(ui, "FOTO CONGELADA", theme::WARN);
        }
    });
    ui.add_space(theme::SPACE_SM);

    (toggle_pause, do_clear)
}
