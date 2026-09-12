//! Painel de serviços: plano legível pretendido × efetivo (só lê).

use eframe::egui;
use orchestrator_studio::services::ServicesPanel;

/// URL, botão de leitura, erro e tabela com a divergência em destaque.
pub fn show(ui: &mut egui::Ui, panel: &mut ServicesPanel) {
    ui.collapsing("Serviços do nó (plano: pretendido × efetivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("nó:");
            ui.text_edit_singleline(&mut panel.url);
            if ui.button("Ler plano").clicked() {
                panel.refresh();
            }
        });
        if !panel.error.is_empty() {
            ui.label(&panel.error);
        }
        if panel.list.is_empty() {
            ui.label("Nenhum serviço lido. Clique Ler plano.");
        } else {
            egui::Grid::new("services_grid").show(ui, |ui| {
                ui.label("serviço");
                ui.label("pretendido");
                ui.label("efetivo");
                ui.label("diff");
                ui.end_row();
                for row in &panel.list {
                    let wanted = match row.wanted {
                        Some(true) => "on",
                        Some(false) => "off",
                        None => "—",
                    };
                    let active = if row.active { "on" } else { "off" };
                    let diff = match row.wanted {
                        Some(w) if w == row.active => "",
                        _ => "⇐ diverge",
                    };
                    ui.label(&row.service);
                    ui.label(wanted);
                    ui.label(active);
                    ui.label(diff);
                    ui.end_row();
                }
            });
        }
    });
}
