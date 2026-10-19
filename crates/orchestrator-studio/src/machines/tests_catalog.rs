use super::*;
use studio_core::catalog::{Cursor, DefinitionId, ItemView};
use studio_core::revision::Revision;

fn machine(id: &str) -> MachineRecord {
    MachineRecord {
        machine_id: String::from(id),
        host: String::from("192.168.1.61"),
        user: String::from("tese"),
        node_url: String::from("http://192.168.1.61:4317"),
        services_hint: Some(String::from("studio-noded,orchestrator")),
        registered_at_ns: 1_759_500_000_000_000_000,
    }
}

#[test]
fn machine_record_round_trips_through_catalog_value() {
    // Given: máquina registrada; When: valor de catálogo ida-e-volta.
    let original = machine("orch-62");
    let value = machine_catalog_value(&original).expect("serializa");
    let parsed = parse_machine_value(&value).expect("envelope machine");
    // Then: registro idêntico; envelope expõe kind; id prefixado.
    assert_eq!(parsed, original);
    assert!(value.contains(r#""kind":"machine""#));
    assert!(!value.contains("token"), "segredo não existe no registro");
    assert_eq!(original.catalog_id(), "machine:orch-62");
}

#[test]
fn parse_rejects_non_machine_entries() {
    // Entradas de outros objetos do catálogo (e lixo) nunca viram máquina.
    assert!(parse_machine_value(r#"{"kind":"agent","agent_id":"a"}"#).is_none());
    assert!(parse_machine_value("texto solto").is_none());
    assert!(parse_machine_value(r#"{"machine":{"machine_id":"x"}}"#).is_none());
    assert!(parse_machine_value("{}").is_none());
}

#[test]
fn machines_from_snapshot_filters_only_machines_and_sorts() {
    let machine_a = machine("orch-b");
    let machine_b = machine("orch-a");
    let snapshot = Snapshot {
        items: vec![
            ItemView {
                id: DefinitionId(String::from("proj-z")),
                value: String::from(r#"{"kind":"agent"}"#),
                revision: Revision(0),
            },
            ItemView {
                id: DefinitionId(machine_b.catalog_id()),
                value: machine_catalog_value(&machine_b).expect("serializa"),
                revision: Revision(0),
            },
            ItemView {
                id: DefinitionId(machine_a.catalog_id()),
                value: machine_catalog_value(&machine_a).expect("serializa"),
                revision: Revision(3),
            },
            ItemView {
                id: DefinitionId(String::from("machine:quebrada")),
                value: String::from("{não é json"),
                revision: Revision(1),
            },
        ],
        cursor: Cursor(4),
    };
    let machines = machines_from_snapshot(&snapshot);
    assert_eq!(
        machines
            .iter()
            .map(|m| m.machine_id.as_str())
            .collect::<Vec<_>>(),
        vec!["orch-a", "orch-b"],
        "só kind==machine, ordenado por machine_id"
    );
}

#[test]
fn debug_never_contains_tokens() {
    // RNF-04: o Debug do estado não pode vazar segredo (nem valor, nem
    // presença por chave de token).
    let mut state = MachinesState::with_url("http://127.0.0.1:4317");
    state.form_token = String::from("segredo-super-forte-123");
    state.authority_token = String::from("autoridade-secreta-456");
    state.tokens.insert(
        String::from("http://192.168.1.61:4317"),
        String::from("outro-segredo-789"),
    );
    let rendered = format!("{state:?}");
    assert!(
        !rendered.contains("segredo"),
        "Debug vazou token: {rendered}"
    );
    assert!(
        !rendered.contains("autoridade"),
        "Debug vazou token: {rendered}"
    );
}

#[test]
fn probe_unreachable_is_offline_honest() {
    // Porta fechada no loopback: recusa determinística de conexão.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback deve ligar");
    let port = listener.local_addr().expect("porta legivel").port();
    drop(listener);
    let status = probe_status(&format!("http://127.0.0.1:{port}"), None);
    assert_eq!(status.state, ProbeState::Offline);
    assert!(
        status.detail.contains("falha ao ler o no"),
        "{}",
        status.detail
    );
}
