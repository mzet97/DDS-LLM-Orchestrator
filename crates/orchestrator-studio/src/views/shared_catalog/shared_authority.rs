//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_authority(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    // ── Autoridade + sincronização ──
    let leader = if shared.url.is_empty() {
        String::from("—")
    } else {
        kit::authority(&shared.url)
    };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("PRIMARY_LEADER: {leader}"))
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
        );
        // right_to_left: primeiro adicionado = mais à direita.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!shared.busy, |ui| {
                if ui.button("Forçar Sincronização").clicked() {
                    shared.force_sync();
                }
                if ui.button("Exportar JSON").clicked() {
                    match shared.export_snapshot() {
                        Ok(path) => {
                            shared.notice = format!("snapshot exportado: {}", path.display());
                        }
                        Err(err) => {
                            shared.notice = format!("exportação falhou: {err}");
                        }
                    }
                }
                if kit::primary_button(ui, "Publicar novo registro").clicked() {
                    shared.new_entry();
                }
                if ui.button("Ler snapshot").clicked() {
                    shared.refresh();
                }
            });
        });
    });
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        ui.label(caption("AUTORIDADE"));
        ui.add(
            egui::TextEdit::singleline(&mut shared.url)
                .desired_width(280.0)
                .hint_text("http://127.0.0.1:4317"),
        );
        ui.label(caption("TOKEN"));
        ui.add(
            egui::TextEdit::singleline(&mut shared.token)
                .password(true)
                .desired_width(160.0)
                .hint_text("só memória"),
        );
        ui.add_enabled_ui(!shared.busy, |ui| {
            if ui.button("Acompanhar eventos").clicked() {
                shared.follow_events();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);
}
