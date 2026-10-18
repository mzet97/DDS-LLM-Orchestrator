//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_context(
    ui: &mut egui::Ui,
    dds: &mut DdsState,
    discovery: &DiscoveryState,
    drained: usize,
) {
    // ── Faixa de contexto (design: strip fino sob o topbar; rótulos reais) ──
    egui::Frame::NONE
        .fill(theme::SURFACE_LOW)
        .inner_margin(egui::Margin::symmetric(12, 5))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .small()
                        .color(theme::PRIMARY_CONTAINER),
                );
                ui.label(
                    egui::RichText::new("TOPOLOGIA DE REDE DDS")
                        .monospace()
                        .size(10.5)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "QoS: RELIABLE + TRANSIENT_LOCAL · DOMÍNIO {}",
                        dds.domain
                    ))
                    .monospace()
                    .size(10.0)
                    .color(theme::OUTLINE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "DRENADOS: {drained} MSG / {}s",
                            dds.window_secs
                        ))
                        .monospace()
                        .size(10.0)
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });
        });
    ui.separator();

    // ── Hero (design: título + ONLINE + domínio/janela/Observar à direita) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("3.11 Topologia DDS & Enxame Distribuído")
                        .size(20.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                let alive = snapshot_has_data(&dds.snapshot) || !discovery.nodes.is_empty();
                kit::badge(
                    ui,
                    if alive { "● ONLINE" } else { "○ OFFLINE" },
                    if alive { theme::OK } else { theme::STALE },
                );
            });
            ui.label(
                egui::RichText::new(
                    "Visualização estrutural de participantes de domínio, discovery \
                     leases, tópicos pub/sub e barramento RTPS.",
                )
                .small()
                .weak(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!dds.busy, |ui| {
                let observe = kit::primary_button(
                    ui,
                    if dds.busy {
                        "Observando domínio…"
                    } else {
                        "Observar Domínio"
                    },
                );
                if observe.clicked() {
                    crate::studio_log::info(format!(
                        "topologia: observação manual do domínio {} (janela {}s)",
                        dds.domain, dds.window_secs
                    ));
                    dds.refresh();
                }
            });
            egui::ComboBox::from_id_salt("topology_window")
                .selected_text(format!("{} segundos", dds.window_secs))
                .show_ui(ui, |ui| {
                    for secs in WINDOW_PRESETS {
                        ui.selectable_value(&mut dds.window_secs, secs, format!("{secs} segundos"));
                    }
                });
            ui.label(
                egui::RichText::new("JANELA DDS:")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.add(egui::DragValue::new(&mut dds.domain).range(0..=230));
            ui.label(
                egui::RichText::new("DOMÍNIO:")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
    if !dds.error.is_empty() && !dds.busy {
        ui.add_space(theme::SPACE_SM);
        kit::error_banner(ui, &dds.error);
    }
    ui.add_space(theme::SPACE_MD);
}
