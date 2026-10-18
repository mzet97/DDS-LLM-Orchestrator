//! Painel 3.8 Serviços do Sistema (systemd): alvo + sincronização, banner
//! de intertravamento do modo protegido, confirmação em 2 passos, plano
//! pretendido × efetivo com comandos remotos e auditoria com relógio.
//!
//! Só dados reais: o fio entrega `{service, wanted, active}` — sem PID,
//! subestado, enabled, descrição ou versão do init (itens do mockup que não
//! existem e ficam FORA). Transporte real: `POST /services/<u>/<start|stop>`.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::panel_header::panel_header;
use crate::protected::ProtectedGuard;
use crate::protected::ProtectedOutcome;
use crate::services::ServicesPanel;
use crate::theme;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    panel: &mut ServicesPanel,
    guard: &mut ProtectedGuard,
    discovery: &DiscoveryState,
) {
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    panel.poll();

    let target = if panel.url.is_empty() {
        String::from("—")
    } else {
        panel.url.clone()
    };
    panel_header(
        ui,
        "SEC 3.8",
        "3.8 Serviços do Sistema (systemd)",
        "Daemon control no nó alvo: plano pretendido × efetivo e atuação remota sob intertravamento.",
    );
    ui.horizontal(|ui| {
        kit::badge(ui, "DAEMON CONTROL", theme::PRIMARY_FIXED_DIM);
        ui.label(
            egui::RichText::new(format!(
                "ALVO: {target} · ● DDS Domain {}",
                discovery.domain
            ))
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let read = ui.add_enabled(!panel.busy, egui::Button::new("↻ Ler plano de serviços"));
            if read.clicked() {
                panel.audit_manual_list = true; // GET manual → auditoria
                panel.refresh();
            }
            ui.label(
                egui::RichText::new(match panel.last_sync_ms {
                    Some(ms) => format!("Última Sincronização: {}", kit::clock_ms(ms)),
                    None => String::from("Última Sincronização: —"),
                })
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        ui.label(caption("NÓ ALVO (HTTP :4317)"));
        ui.add_enabled_ui(!panel.busy, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut panel.url)
                    .desired_width(300.0)
                    .hint_text("http://127.0.0.1:4317"),
            );
        });
    });
    if !panel.error.is_empty()
        && panel.error != "lendo plano…"
        && panel.error != "atuando…"
        && !panel.error.starts_with("reiniciando")
    {
        kit::error_banner(ui, &panel.error);
    } else if panel.busy {
        ui.label(
            egui::RichText::new(format!("○ {}", panel.error))
                .small()
                .color(theme::WARN),
        );
    }
    ui.add_space(theme::SPACE_SM);

    show_banner(ui, panel, guard, discovery);
    show_confirm(ui, panel, guard, discovery);
    show_plan(ui, panel, guard, discovery);
    show_audit(ui, panel, guard, discovery);
}

mod services_audit;
mod services_banner;
mod services_confirm;
mod services_helpers;
mod services_plan;

use services_audit::show_audit;
use services_banner::show_banner;
use services_confirm::show_confirm;
use services_helpers::{caption, cmd_button};
use services_plan::show_plan;
