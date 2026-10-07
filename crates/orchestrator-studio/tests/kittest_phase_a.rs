//! Fase A (vitrine): verificação headless dos painéis redesenhados.

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::DiscoveryState;
use orchestrator_studio::machines::MachinesState;
use orchestrator_studio::views;

fn discovered(n: usize) -> DiscoveryState {
    let mut state = DiscoveryState::new_disabled(170);
    state.observe(
        (0..n)
            .map(|i| orchestrator_studio::discovery::DiscoveredNode {
                node_id: format!("192.168.1.6{}:4317", i + 1),
                url: format!("http://192.168.1.6{}:4317", i + 1),
                token_required: true,
                last_seen_unix_ns: orchestrator_studio::machines::now_unix_ns(),
                probe: None,
            })
            .collect(),
    );
    state
}

#[test]
fn machines_hero_renders_lease_header_and_cards() {
    let mut machines = MachinesState::with_url("http://127.0.0.1:1");
    let mut discovery = discovered(3);
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (machines, discovery)| views::machines::show(ui, machines, discovery),
        (&mut machines, &mut discovery),
    );
    harness.set_size(egui::vec2(1000.0, 700.0));
    harness.run_steps(5);
    harness.get_by_label_contains("Escutando tráfego Studio.NodePresence");
}
