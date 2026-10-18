//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_hero(
    ui: &mut egui::Ui,
    machines: &mut MachinesState,
    discovery: &mut DiscoveryState,
    domain: u32,
    now_ns: u64,
) {
    // ── Hero (mockup: eyebrow + título + stats + FORÇAR à direita) ──
    let total = discovery.nodes.len();
    let alive = discovery
        .nodes
        .iter()
        .filter(|node| node.is_alive(now_ns))
        .count();
    let freshest_age = discovery
        .nodes
        .iter()
        .map(|node| node.age_secs(now_ns))
        .min();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("SEÇÃO 3.10 · DESCOBERTA AUTOMÁTICA")
                        .monospace()
                        .size(10.5)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                kit::badge(ui, "TOPIC: Studio.NodePresence", theme::OUTLINE);
                kit::badge(
                    ui,
                    &format!("DOMÍNIO DDS {domain}"),
                    theme::PRIMARY_FIXED_DIM,
                );
            });
            ui.label(
                egui::RichText::new(format!("Descoberta de Nós no Domínio DDS {domain}"))
                    .size(22.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .size(12.0)
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                // Âncora kittest: "Escutando tráfego Studio.NodePresence".
                ui.label(
                    egui::RichText::new(
                        "Escutando tráfego Studio.NodePresence · Lease 10s · Poda 30s · \
                         Heartbeat a cada 5s",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!machines.busy && total > 0, |ui| {
                let force = ui.button(
                    egui::RichText::new("FORÇAR PROBE MULTICAST")
                        .monospace()
                        .strong(),
                );
                if force.clicked() {
                    let urls: Vec<String> = discovery.nodes.iter().map(|n| n.url.clone()).collect();
                    machines.probe_all(&urls);
                    crate::studio_log::info(format!(
                        "máquinas: probe forçado de {} nó(s) pelo usuário",
                        urls.len()
                    ));
                }
            });
            // Stats em caixas FIXAS com layout explícito: `vertical` dentro
            // de horizontal `right_to_left` reivindica toda a largura e os
            // irmãos seguintes colapsam (rótulos saem na vertical).
            hero_stat(ui, 150.0, "CICLO DE VARREDURA", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{HEARTBEAT_SECS}.0s"))
                            .size(17.0)
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    match freshest_age {
                        Some(age) => {
                            let next = HEARTBEAT_SECS.saturating_sub(age);
                            ui.label(
                                egui::RichText::new(format!("próx: {next}.0s"))
                                    .monospace()
                                    .small()
                                    .color(theme::PRIMARY_FIXED_DIM),
                            );
                        }
                        None => {
                            ui.label(egui::RichText::new("próx: —").monospace().small().weak());
                        }
                    }
                });
            });
            hero_stat(ui, 165.0, "PARTICIPANTES ATIVOS", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{alive}/ "))
                            .size(17.0)
                            .strong()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    ui.label(
                        egui::RichText::new(format!("{total} nós"))
                            .size(14.0)
                            .color(theme::ON_SURFACE),
                    );
                });
            });
        });
    });
    ui.add_space(theme::SPACE_SM);

    if discovery.scanning && discovery.nodes.is_empty() {
        kit::empty_state(ui, "escutando o domínio…");
    }
    if !discovery.error.is_empty() {
        kit::error_banner(ui, &discovery.error);
    }
}
