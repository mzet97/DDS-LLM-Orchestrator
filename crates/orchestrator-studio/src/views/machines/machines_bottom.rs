//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

use super::machines_form::{show_catalog, show_manual_form};

pub(super) fn show_bottom(
    ui: &mut egui::Ui,
    machines: &mut MachinesState,
    discovery: &mut DiscoveryState,
    _domain: u32,
    _now_ns: u64,
) {
    // ── Linha de 2 cards: registro manual · catálogo persistido ──
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            show_manual_form(ui, machines);
        });
        cols[1].vertical(|ui| {
            show_catalog(ui, machines);
        });
    });

    // ── Modal: Definir Alvo & Inserir Token ──
    let modal_url = machines.modal_url.clone();
    if let Some(url) = modal_url {
        let mut close = false;
        egui::Window::new("Definir Alvo & Inserir Token")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(egui::RichText::new("URL DO NÓ:").monospace().small());
                kit::mono_cell(ui, &url);
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    egui::RichText::new("TOKEN BEARER (fica só na memória da GUI):")
                        .monospace()
                        .small(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut machines.modal_token)
                        .password(true)
                        .desired_width(320.0),
                );
                ui.add_space(theme::SPACE_SM);
                ui.horizontal(|ui| {
                    let confirm = ui.add_enabled(
                        !machines.modal_token.trim().is_empty(),
                        egui::Button::new(
                            egui::RichText::new("Validar & Comutar Alvo").color(theme::ON_PRIMARY),
                        )
                        .fill(theme::PRIMARY_CONTAINER),
                    );
                    if confirm.clicked() {
                        let token = machines.modal_token.trim().to_owned();
                        machines.remember_token(&url, &token);
                        machines.probe_url(&url);
                        // Comuta o alvo único da GUI para este nó.
                        if let Some(index) = discovery.nodes.iter().position(|n| n.url == url) {
                            discovery.selected = Some(index);
                        }
                        crate::studio_log::info(format!(
                            "máquinas: token guardado em memória + alvo comutado para {url}"
                        ));
                        close = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            machines.modal_url = None;
            machines.modal_token.clear();
        }
    }
}
