//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_feed(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    // ── Feed de mutações (sempre visível; relógio + etiqueta por linha) ──
    ui.add_space(theme::SPACE_MD);
    ui.horizontal(|ui| {
        ui.label(header("FEED DE MUTAÇÕES INCREMENTAIS"));
        ui.label(
            egui::RichText::new("CANAL: HTTP /catalog/events (cursor)")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clear = ui.add_enabled(
                !shared.feed.is_empty(),
                egui::Button::new("Limpar visualização"),
            );
            if clear.clicked() {
                shared.feed.clear();
            }
            ui.label(
                egui::RichText::new(format!("BUFFER: {} EVTS", shared.feed.len()))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    if shared.feed.is_empty() {
        kit::empty_state(ui, "nenhum evento acompanhado nesta sessão.");
    } else {
        egui::Frame::NONE
            .fill(theme::SURFACE_LOW)
            .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
            .inner_margin(theme::SPACE_MD)
            .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for entry in shared.feed.iter().rev().take(10) {
                    let tag_color = match entry.tag {
                        FeedTag::Committed => theme::PRIMARY_FIXED_DIM,
                        FeedTag::Tombstone => theme::WARN,
                        FeedTag::Rejected => theme::ERROR,
                        FeedTag::Snapshot => theme::STALE,
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("[{}]", kit::clock_ms(entry.ts_ms)))
                                .monospace()
                                .small()
                                .color(theme::OUTLINE),
                        );
                        ui.label(
                            egui::RichText::new(entry.tag.label())
                                .monospace()
                                .small()
                                .strong()
                                .color(tag_color),
                        );
                        ui.label(
                            egui::RichText::new(&entry.text)
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                }
            });
    }
}
