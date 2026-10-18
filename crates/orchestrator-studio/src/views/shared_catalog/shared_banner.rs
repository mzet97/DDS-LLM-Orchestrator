//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_banner(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    // ── Banner 409 estruturado (base tentada × vigente do nó) ──
    if conflict_active(shared) {
        let (base_text, current_text) = match shared.conflict {
            Some(info) => (
                info.attempted_base
                    .map_or(String::from("—"), |b| format!("rev.{b}")),
                info.current
                    .map_or(String::from("—"), |c| format!("rev.{c}")),
            ),
            None => (String::from("?"), String::from("?")),
        };
        egui::Frame::NONE
            .fill(theme::tint(theme::ERROR, 10))
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::ERROR))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("HTTP 409 CONFLICT: REVISÃO CONFLITANTE")
                                    .monospace()
                                    .strong()
                                    .color(theme::ERROR),
                            );
                            ui.label(
                                egui::RichText::new(format!("Base: {base_text}"))
                                    .monospace()
                                    .small()
                                    .color(theme::ERROR),
                            );
                            ui.label(
                                egui::RichText::new(format!("Servidor: {current_text}"))
                                    .monospace()
                                    .small()
                                    .color(theme::ERROR),
                            );
                        });
                        ui.label(
                            egui::RichText::new(
                                "Conflito de concorrência otimista (OCC): o registro foi \
                                 modificado por outro escritor antes da sua publicação. \
                                 Recarregue o snapshot e republique com a revisão vigente — \
                                 nada foi aplicado.",
                            )
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        if !shared.notice.is_empty() {
                            ui.label(
                                egui::RichText::new(&shared.notice)
                                    .monospace()
                                    .small()
                                    .color(theme::WARN),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(!shared.busy, |ui| {
                            if ui.button("Atualizar Snapshot & Mesclar").clicked() {
                                shared.refresh();
                            }
                        });
                    });
                });
            });
        ui.add_space(theme::SPACE_MD);
    } else if !shared.notice.is_empty() {
        ui.label(egui::RichText::new(&shared.notice).small().weak());
        ui.add_space(theme::SPACE_SM);
    }
}
