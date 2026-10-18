//! Parte de `views::models` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_artifacts(ui: &mut egui::Ui, state: &mut ModelsState) {
    let deviants: Vec<&str> = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Desviado)
        .map(|item| item.file_name.as_str())
        .collect();
    let pending_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Pendente)
        .count();
    // ── Artefatos + filtros por status (mockup 3.7) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Artefatos Armazenados")
                .strong()
                .color(theme::ON_SURFACE_VARIANT),
        );
        ui.label(
            egui::RichText::new(format!("{} Arquivos Mapeados", state.list.len()))
                .small()
                .weak(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (id, label) in [
                (2u8, format!("Pendentes ({pending_count})")),
                (1u8, format!("Divergentes ({})", deviants.len())),
                (0u8, format!("Todos ({})", state.list.len())),
            ] {
                if ui
                    .selectable_label(
                        state.filter == id,
                        egui::RichText::new(label).monospace().small(),
                    )
                    .clicked()
                {
                    state.filter = id;
                }
            }
            ui.label(
                egui::RichText::new("FILTRO:")
                    .small()
                    .strong()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    ui.add_space(theme::SPACE_SM);
}
