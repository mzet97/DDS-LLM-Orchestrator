//! Fase A (vitrine): verificação headless dos painéis redesenhados.

use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::DiscoveryState;
use orchestrator_studio::machines::{MachinesState, ProbeState, ProbeStatus};
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

fn probed(url: &str, state: ProbeState, detail: &str) -> DiscoveryState {
    let mut discovery = DiscoveryState::new_disabled(170);
    discovery.observe(vec![orchestrator_studio::discovery::DiscoveredNode {
        node_id: String::from("nodo-de-prova"),
        url: String::from(url),
        token_required: true,
        last_seen_unix_ns: orchestrator_studio::machines::now_unix_ns(),
        probe: Some(ProbeStatus {
            state,
            detail: String::from(detail),
        }),
    }]);
    discovery
}

fn harness_310<'a>(
    machines: &'a mut MachinesState,
    discovery: &'a mut DiscoveryState,
) -> egui_kittest::Harness<'a, (&'a mut MachinesState, &'a mut DiscoveryState)> {
    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (machines, discovery)| views::machines::show(ui, machines, discovery),
        (machines, discovery),
    );
    harness.set_size(egui::vec2(1400.0, 900.0));
    harness.run_steps(5);
    harness
}

#[test]
fn machines_auth_row_renders_401_pill_and_token_action() {
    let mut machines = MachinesState::with_url("http://127.0.0.1:1");
    let mut discovery = probed(
        "http://127.0.0.1:4318",
        ProbeState::AuthPending,
        "token ausente (nó respondeu 401)",
    );
    let harness = harness_310(&mut machines, &mut discovery);
    // (pills `kit::badge` são pintadas à mão — invisíveis ao kittest; o texto
    // delas é coberto por testes unitários em `views::machines`.)
    harness.get_by_label_contains("Canal RPC aberto");
    harness.get_by_label_contains("× Não informado");
    harness.get_by_label_contains("Definir Alvo & Inserir Token");
    harness.get_by_label_contains("Requer Bearer Token");
}

#[test]
fn machines_offline_row_renders_refused_port_line() {
    let mut machines = MachinesState::with_url("http://127.0.0.1:1");
    let mut discovery = probed(
        "http://127.0.0.1:4317",
        ProbeState::Offline,
        "connection refused",
    );
    let harness = harness_310(&mut machines, &mut discovery);
    harness.get_by_label_contains("connection refused");
    harness.get_by_label_contains("Sem conexão de controle");
    harness.get_by_label_contains("Re-testar Conexão");
}

#[test]
fn machines_online_row_renders_protocol_and_target() {
    let mut machines = MachinesState::with_url("http://127.0.0.1:1");
    let mut discovery = probed("http://127.0.0.1:4317", ProbeState::Online, "protocolo 1.0");
    discovery.selected = Some(0);
    let harness = harness_310(&mut machines, &mut discovery);
    harness.get_by_label_contains("protocolo 1.0 (OK)");
    harness.get_by_label_contains("● ATIVO");
    harness.get_by_label_contains("Alvo Atual");
}

#[test]
fn machines_token_modal_renders_when_open() {
    let mut machines = MachinesState::with_url("http://127.0.0.1:1");
    machines.modal_url = Some(String::from("http://127.0.0.1:4318"));
    let mut discovery = DiscoveryState::new_disabled(170);
    let harness = harness_310(&mut machines, &mut discovery);
    harness.get_by_label_contains("TOKEN BEARER");
    harness.get_by_label_contains("Validar & Comutar Alvo");
}
