//! Parte de `views::topology` (extraída para manter os arquivos revisáveis).

use super::*;

/// Aba Métricas (SystemMetric drenadas na janela).
pub(super) fn metrics_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.metrics.is_empty() {
        kit::empty_state(ui, "Nenhuma métrica de sistema na janela.");
        return;
    }
    kit::table("topology_metrics_grid").show(ui, |ui| {
        kit::grid_header(ui, &["Origem", "Métrica (unidade)", "Valor"]);
        for metric in &snapshot.metrics {
            if !filter.is_empty() && !matches(&[&metric.source, &metric.name]) {
                continue;
            }
            kit::mono_cell(ui, &metric.source);
            ui.label(&metric.name);
            kit::num_cell(ui, &format!("{:.3}", metric.value));
            ui.end_row();
        }
    });
}

/// Aba Descoberta DDS (eventos drenados na janela).
pub(super) fn discovery_tab(
    ui: &mut egui::Ui,
    snapshot: &DdsSnapshot,
    filter: &str,
    matches: impl Fn(&[&str]) -> bool,
) {
    if snapshot.discoveries.is_empty() {
        kit::empty_state(ui, "Nenhum evento de descoberta na janela.");
        return;
    }
    kit::table("topology_discovery_grid").show(ui, |ui| {
        kit::grid_header(ui, &["Evento", "Tópico", "Entidade remota"]);
        for event in &snapshot.discoveries {
            if !filter.is_empty()
                && !matches(&[&event.event_type, &event.topic_name, &event.remote_entity])
            {
                continue;
            }
            ui.label(&event.event_type);
            kit::mono_cell(ui, &event.topic_name);
            kit::mono_cell(ui, &event.remote_entity);
            ui.end_row();
        }
    });
}

/// Aba Instalações (presença contínua: nós Studio com probe e idade).
pub(super) fn installations_tab(ui: &mut egui::Ui, discovery: &DiscoveryState) {
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma instalação vista no domínio — nós com studio-node publicam \
             presença a cada 5 s.",
        );
        return;
    }
    let now = now_unix_ns();
    kit::table("topology_nodes_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Node ID",
                "URL",
                "Estado do probe",
                "Idade HB",
                "Token exigido",
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
            kit::num_cell(ui, &format!("{} s", node.age_secs(now)));
            ui.label(if node.token_required { "sim" } else { "não" });
            ui.end_row();
        }
    });
}
