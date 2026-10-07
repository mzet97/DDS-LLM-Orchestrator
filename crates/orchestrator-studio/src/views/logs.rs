//! Painel de Logs: eventos do Studio (descoberta, auto-carga, observação)
//! em buffer circular — comportamento analisável dentro da própria GUI.

use crate::studio_log;
use crate::theme;
use eframe::egui;

pub fn show(ui: &mut egui::Ui) {
    let entries = studio_log::entries();
    let info_count = entries.iter().filter(|e| e.level == "INFO").count();
    let warn_count = entries.iter().filter(|e| e.level == "WARN").count();
    let error_count = entries.iter().filter(|e| e.level == "ERRO").count();

    ui.heading(format!(
        "FIFO Ring Buffer [500] — {}/500 slots",
        entries.len()
    ));
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("● INFO {info_count}"))
                .color(egui::Color32::from_rgb(140, 210, 170)),
        );
        ui.label(egui::RichText::new(format!("◐ WARN {warn_count}")).color(theme::WARN));
        ui.label(egui::RichText::new(format!("● ERRO {error_count}")).color(theme::ERROR));
    });
    ui.label(
        "Eventos de descoberta, seleção de alvo, auto-carga e observação. \
         Os mesmos eventos vão para o stderr do processo.",
    );
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if entries.is_empty() {
                ui.label("(nenhum evento ainda)");
            }
            egui::Grid::new("studio_logs").show(ui, |ui| {
                ui.strong("hora");
                ui.strong("nível");
                ui.strong("evento");
                ui.end_row();
                for entry in &entries {
                    ui.monospace(&entry.timestamp);
                    let color = match entry.level {
                        "ERRO" => egui::Color32::RED,
                        "WARN" => egui::Color32::YELLOW,
                        _ => egui::Color32::from_rgb(140, 210, 170),
                    };
                    ui.label(egui::RichText::new(entry.level).color(color));
                    ui.label(&entry.message);
                    ui.end_row();
                }
            });
        });
}
