//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_chips(ui: &mut egui::Ui, dds: &mut DdsState) {
    let tools = &dds.snapshot.tools;
    let denied_total = tools.iter().filter(|t| t.status == 2).count();
    // ── Chips de status + taxa REQ/s da janela + bloqueios da sessão ──
    // (taxa honesta: chamadas drenadas ÷ janela — não é QPS do barramento)
    let rate = tools.len() as f64 / dds.window_secs.max(1) as f64;
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(
                dds.tools_filter.is_none(),
                egui::RichText::new(format!("Todos ({})", tools.len()))
                    .monospace()
                    .small(),
            )
            .clicked()
        {
            dds.tools_filter = None;
        }
        for (code, name) in STATUS_FILTERS {
            let count = tools.iter().filter(|t| t.status == code).count();
            if ui
                .selectable_label(
                    dds.tools_filter == Some(code),
                    egui::RichText::new(format!("{name} ({count})"))
                        .monospace()
                        .small(),
                )
                .clicked()
            {
                dds.tools_filter = Some(code);
            }
        }
        ui.separator();
        ui.label(
            egui::RichText::new(format!("TAXA {rate:.1} REQ/s (janela)"))
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
        );
        ui.separator();
        ui.label(
            egui::RichText::new(format!("BLOQUEIOS (DENIED na janela): {denied_total}"))
                .monospace()
                .small()
                .color(if denied_total > 0 {
                    theme::ERROR
                } else {
                    theme::OK
                }),
        );
    });
    ui.add_space(theme::SPACE_SM);
}
