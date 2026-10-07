//! Painel de inferência: sessão multi-turn contra o llama ao vivo.

use crate::discovery::DiscoveryState;
use crate::inference::{InferenceState, Role};
use crate::kit;
use eframe::egui;

/// Servidor, modelos, parâmetros, prompt e transcript da sessão.
pub fn show(ui: &mut egui::Ui, infer: &mut InferenceState, discovery: &DiscoveryState) {
    // Servidores de inferência VIVOS no domínio (ServerStatus — descoberta
    // automática). Nota honesta: o contrato não carrega a URL HTTP do
    // servidor (só id/modelo/slots); a URL do formulário segue manual.
    // ── PLANO 1 (3.3): SERVIDOR DESCOBERTO — DDS, automático ──
    ui.collapsing(
        format!(
            "SERVIDOR DESCOBERTO (DDS) — domínio {} [{}]",
            discovery.domain,
            if discovery.servers.is_empty() { "nenhum" } else { "presente" }
        ),
        |ui| {
            if discovery.servers.is_empty() {
                ui.label(format!(
                    "Nenhum servidor anunciando ServerStatus no domínio {} — \
                     o llama-server precisa rodar com --enable-dds --dds-domain {} \
                     para aparecer aqui.",
                    discovery.domain, discovery.domain
                ));
            } else {
                egui::Grid::new("dds_live_servers")
                    .striped(true)
                    .show(ui, |ui| {
                        kit::grid_header(ui, &["server_id", "modelo", "slots", "pronto"]);
                        for server in &discovery.servers {
                            kit::mono_cell(ui, &server.server_id);
                            ui.label(&server.model_loaded);
                            ui.label(format!(
                                "{}/{}",
                                server.slots_processing,
                                server.slots_idle + server.slots_processing
                            ));
                            ui.label(if server.ready { "sim" } else { "não" });
                            ui.end_row();
                        }
                    });
                ui.weak("Nota: o ServerStatus não carrega URL HTTP — o endpoint do chat é manual (abaixo).");
            }
        },
    );
    ui.separator();
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    infer.poll();
    ui.collapsing("Inferência (servidor llama ao vivo)", |ui| {
        ui.horizontal(|ui| {
            ui.label("servidor:");
            ui.add_enabled_ui(!infer.busy, |ui| {
                ui.text_edit_singleline(&mut infer.server_url);
                if ui.button("Modelos").clicked() {
                    infer.refresh_models();
                }
            });
        });
        if infer.models.is_empty() {
            ui.text_edit_singleline(&mut infer.model);
        } else {
            egui::ComboBox::from_label("modelo")
                .selected_text(&infer.model)
                .show_ui(ui, |ui| {
                    for candidate in infer.models.clone() {
                        ui.selectable_value(&mut infer.model, candidate.clone(), candidate);
                    }
                });
        }
        ui.add(egui::Slider::new(&mut infer.temperature, 0.0..=2.0).text("temperatura"));
        ui.add(egui::Slider::new(&mut infer.max_tokens, 1..=4096).text("máx. tokens"));
        ui.label("prompt:");
        ui.text_edit_multiline(&mut infer.prompt);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!infer.busy, |ui| {
                if ui.button("Enviar").clicked() {
                    infer.send();
                }
            });
            if ui.button("Nova sessão").clicked() {
                infer.clear_session();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                if infer.history.is_empty() {
                    ui.label(&infer.reply);
                }
                for message in &infer.history {
                    let who = match message.role {
                        Role::System => "sistema",
                        Role::User => "você",
                        Role::Assistant => "assistente",
                    };
                    ui.label(format!("{who}: {}", message.content));
                }
            });
    });
}
