//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_tabs(ui: &mut egui::Ui, dds: &mut DdsState, discovery: &DiscoveryState) {
    // ── Painel de 7 abas com filtro (UX3: pills) ──
    ui.horizontal(|ui| {
        for (index, name) in TABS.iter().enumerate() {
            let active = dds.tab as usize == index;
            if kit::pill_chip(ui, active, name, theme::PRIMARY_FIXED_DIM).clicked() {
                dds.tab = index as u8;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut dds.filter)
                    .hint_text("Filtrar por ID, tópico ou entidade…")
                    .desired_width(240.0)
                    .font(egui::TextStyle::Monospace),
            );
        });
    });
    ui.add_space(theme::SPACE_SM);
    let filter = dds.filter.trim().to_lowercase();
    let matches = |parts: &[&str]| {
        filter.is_empty()
            || parts
                .iter()
                .any(|part| part.to_lowercase().contains(&filter))
    };
    let tab = dds.tab;
    let snapshot = &dds.snapshot;
    match tab {
        0 => tasks_tab(ui, snapshot, &mut dds.task_selected, &filter, matches),
        1 => agents_tab(ui, discovery),
        2 => servers_tab(ui, discovery),
        3 => tools_tab(ui, snapshot, &filter, matches),
        4 => metrics_tab(ui, snapshot, &filter, matches),
        5 => discovery_tab(ui, snapshot, &filter, matches),
        _ => installations_tab(ui, discovery),
    }

    // ── Barra de log de tarefas (TaskOutput ao vivo — rodapé da 3.11) ──
    task_log_bar(ui, snapshot, &mut dds.task_selected);
}
