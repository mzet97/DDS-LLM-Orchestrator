//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_controls(ui: &mut egui::Ui, dds: &mut DdsState, guard: &ProtectedGuard) {
    // ── Controles: janela + observação + export + estado do interlock ──
    // FORA: GOVERNANÇA RBAC_STRICT (a política real é estática/permissiva —
    // M2) e "Pausar Captura" (sem streaming contínuo; a captura é o
    // `Observar Domínio`).
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("JANELA:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        // Opções incluem o default real (5s) + as do PNG.
        let mut picked: Option<u64> = None;
        egui::ComboBox::from_id_salt("tools_window")
            .selected_text(format!("Últimos {} segundos", dds.window_secs))
            .width(200.0)
            .show_ui(ui, |ui| {
                for secs in [5_u64, 10, 30, 60] {
                    if ui
                        .selectable_label(
                            dds.window_secs == secs,
                            format!("Últimos {secs} segundos"),
                        )
                        .clicked()
                    {
                        picked = Some(secs);
                    }
                }
            });
        if let Some(secs) = picked {
            dds.window_secs = secs;
        }
        ui.add_enabled_ui(!dds.busy, |ui| {
            // Primário ciano (padrão 3.9): sem fill parecia desabilitado.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(if dds.busy {
                            "observando…"
                        } else {
                            "Observar Domínio"
                        })
                        .monospace()
                        .small()
                        .color(theme::ON_PRIMARY),
                    )
                    .fill(theme::PRIMARY_CONTAINER)
                    .corner_radius(egui::CornerRadius::same(theme::RADIUS_PILL as u8)),
                )
                .clicked()
            {
                dds.refresh();
            }
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("Exportar JSON").monospace().small(),
                ))
                .clicked()
            {
                match dds.export_tools() {
                    Ok(path) => crate::studio_log::info(format!(
                        "ferramentas: {} chamada(s) exportadas para {}",
                        dds.snapshot.tools.len(),
                        path.display()
                    )),
                    Err(err) => {
                        dds.error = format!("exportação falhou: {err}");
                    }
                }
            }
        });
        kit::badge(
            ui,
            if guard.armed {
                "INTERLOCK: ARMADO"
            } else {
                "INTERLOCK: DESARMADO"
            },
            if guard.armed {
                theme::ERROR
            } else {
                theme::STALE
            },
        );
    });
    if !dds.error.is_empty() && !dds.busy {
        kit::error_banner(ui, &dds.error);
    }
    ui.add_space(theme::SPACE_MD);
}
