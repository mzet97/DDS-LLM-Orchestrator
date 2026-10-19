//! Parte de `views::services` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_confirm(
    ui: &mut egui::Ui,
    panel: &mut ServicesPanel,
    guard: &mut ProtectedGuard,
    _discovery: &DiscoveryState,
) {
    // ── Confirmação da ação pendente (fora do collapsing: banner vermelho
    //    em largura total — âncora literal preservada) ──
    if let Some(pending) = &guard.pending {
        crate::kit::danger_banner(
            ui,
            &format!(
                "CONFIRMAÇÃO EM 2 PASSOS — Ação solicitada: {}",
                pending.description
            ),
        );
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("× Cancelar").clicked() {
                    guard.cancel_pending();
                    panel.pending_action = None;
                    panel.pending_restart = None;
                }
                if ui.button("✓ Confirmar").clicked() {
                    guard.confirm_pending();
                    if let Some(service) = panel.pending_restart.take() {
                        panel.restart_row(&service);
                    } else if let Some((service, start)) = panel.pending_action.take() {
                        panel.actuate_row(&service, start);
                    }
                }
            });
        });
        ui.add_space(theme::SPACE_SM);
    }
}
