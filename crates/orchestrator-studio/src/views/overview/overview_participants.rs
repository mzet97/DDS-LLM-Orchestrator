//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn show_participants(
    ui: &mut egui::Ui,
    _state: &mut AppState,
    _services: &mut ServicesPanel,
    _agents: &mut AgentsState,
    _models: &ModelsState,
    _shared: &mut SharedCatalog,
    _proof: &str,
    discovery: &DiscoveryState,
) {
    // ── Participantes do barramento (design: 5 colunas + domínio) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Topologia de Barramento DDS · Participantes & Partições")
                .size(14.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "DOMÍNIO {} · {} PARTICIPANTES",
                    discovery.domain,
                    discovery.nodes.len()
                ))
                .monospace()
                .size(11.0)
                .color(theme::OUTLINE),
            );
        });
    });
    ui.add_space(theme::SPACE_SM);
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhum participante visto no domínio {} — nós com canvas DDS \
                 publicam presença sozinhos.",
                discovery.domain
            ),
        );
    } else {
        let now = now_unix_ns();
        kit::table("overview_nodes_grid").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "NÓ / PARTICIPANTE",
                    "ENDEREÇO / PORTA",
                    "PROBE",
                    "ÚLTIMO HEARTBEAT",
                    "ESTADO QoS",
                ],
            );
            for node in &discovery.nodes {
                kit::mono_cell(ui, &node.node_id);
                kit::mono_cell(ui, &node.url);
                let (color, detail) = match &node.probe {
                    Some(probe) => match probe.state {
                        ProbeState::Online => (theme::OK, probe.detail.clone()),
                        ProbeState::AuthPending => (theme::AUTH, probe.detail.clone()),
                        ProbeState::Offline | ProbeState::Unknown => {
                            (theme::ERROR, probe.detail.clone())
                        }
                    },
                    None => (theme::STALE, String::from("sondando…")),
                };
                ui.label(
                    egui::RichText::new(format!("● {detail}"))
                        .monospace()
                        .small()
                        .color(color),
                );
                kit::num_cell(ui, &format!("há {}s", node.age_secs(now)));
                kit::mono_cell(ui, "RELIABLE / TRANSIENT");
                ui.end_row();
            }
        });
    }
}
