//! Parte de `views::services` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_banner(
    ui: &mut egui::Ui,
    _panel: &mut ServicesPanel,
    guard: &mut ProtectedGuard,
    _discovery: &DiscoveryState,
) {
    // ── Banner de intertravamento — espelha o guard global ──
    let armed = guard.armed;
    let (banner_fill, banner_stroke) = if armed {
        (theme::tint(theme::ERROR, 10), theme::ERROR)
    } else {
        (theme::SURFACE_HIGH, theme::SURFACE_HIGHEST)
    };
    egui::Frame::NONE
        .fill(banner_fill)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_MD)
        .stroke(egui::Stroke::new(1.0, banner_stroke))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("INTERTRAVAMENTO DE MODO PROTEGIDO")
                                .strong()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        if armed {
                            kit::badge(ui, "ARMADO (GRAVAÇÃO ATIVA)", theme::ERROR);
                        } else {
                            kit::badge(ui, "DESARMADO (LEITURA SOMENTE)", theme::STALE);
                        }
                    });
                    ui.label(
                        egui::RichText::new(
                            "Comandos de mutação remota (start, stop, restart) no nó via \
                             systemd requerem armamento explícito do intertravamento.",
                        )
                        .small()
                        .weak(),
                    );
                    if let Some(refusal) = &guard.last_refusal {
                        if !armed {
                            ui.label(egui::RichText::new(refusal).small().color(theme::WARN));
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle = ui.add(egui::Button::new(
                        egui::RichText::new(if armed {
                            "DISARM / PROTEGER NÓ"
                        } else {
                            "ARM / HABILITAR ATUAÇÃO"
                        })
                        .monospace()
                        .small()
                        .color(if armed {
                            theme::ON_PRIMARY
                        } else {
                            theme::WARN
                        })
                        .strong(),
                    ));
                    if toggle.clicked() {
                        guard.set_armed(!armed);
                    }
                });
            });
        });
    ui.add_space(theme::SPACE_SM);
}
