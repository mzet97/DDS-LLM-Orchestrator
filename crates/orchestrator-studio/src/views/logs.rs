//! Painel de Logs: eventos do Studio (descoberta, auto-carga, observação)
//! em buffer circular — comportamento analisável dentro da própria GUI.

use crate::studio_log;
use eframe::egui;

pub fn show(ui: &mut egui::Ui) {
    let entries = studio_log::entries();
    ui.heading(format!("Logs do Studio ({})", entries.len()));
    ui.label(
        "Eventos de descoberta, seleção de alvo, auto-carga e observação. \
         Os mesmos eventos vão para o stderr do processo (buffer: últimos 500).",
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
