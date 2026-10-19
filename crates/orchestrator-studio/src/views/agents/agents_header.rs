//! Parte de `views::agents` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_header(ui: &mut egui::Ui, agents: &mut AgentsState, discovery: &DiscoveryState) {
    // ── Cabeçalho (mockup) + fontes ──
    ui.horizontal(|ui| {
        kit::badge(ui, "SEC 3.5 // ENXAME DE AGENTES", theme::PRIMARY_FIXED_DIM);
        ui.label(
            egui::RichText::new(
                "Topologia Híbrida: RTPS Multicast Mesh · Orquestrador de Despacho (:8080)",
            )
            .size(15.0)
            .strong()
            .color(theme::ON_SURFACE),
        );
    });
    ui.add_space(theme::SPACE_XS);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(if discovery.scanning {
                "○ FONTE 1: DDS RTPS (DESCOBRINDO…)"
            } else {
                "● FONTE 1: DDS RTPS (DISCOVERY OK)"
            })
            .monospace()
            .small()
            .strong()
            .color(if discovery.scanning {
                theme::WARN
            } else {
                theme::OK
            }),
        );
        let (fonte2, fonte2_color) = if agents.busy {
            ("○ FONTE 2: HTTP :8080 (LENDO…)", theme::WARN)
        } else if agents.last_ok && agents.error.is_empty() {
            ("● FONTE 2: HTTP :8080 (SYNCED)", theme::OK)
        } else if agents.error.is_empty() {
            ("○ FONTE 2: HTTP :8080 (NÃO LIDO)", theme::STALE)
        } else {
            ("○ FONTE 2: HTTP :8080 (FALHOU)", theme::WARN)
        };
        ui.label(
            egui::RichText::new(fonte2)
                .monospace()
                .small()
                .strong()
                .color(fonte2_color),
        );
        if ui
            .selectable_label(agents.simulate_outage, "Simular Queda :8080")
            .clicked()
        {
            agents.toggle_outage();
        }
    });
    ui.add_space(theme::SPACE_XS);
    ui.label(
        egui::RichText::new(
            "SEMÂNTICA DUAL: os agentes descobertos via RTPS operam em barramento \
             peer-to-peer autônomo (alta tolerância). A telemetria HTTP é auxiliar \
             e NÃO introduz ponto único de falha. QoS: TRANSIENT_LOCAL / RELIABLE",
        )
        .small()
        .color(theme::ON_SURFACE_VARIANT),
    );
    ui.add_space(theme::SPACE_LG);
}
