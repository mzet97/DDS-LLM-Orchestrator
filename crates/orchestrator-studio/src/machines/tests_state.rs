use super::*;
use studio_core::catalog::{Cursor, DefinitionId, ItemView, Snapshot};
use studio_core::revision::Revision;

#[test]
fn remove_machine_without_snapshot_is_honest_noop() {
    // Sem snapshot: nada é enviado, aviso explica o pré-requisito.
    let mut state = MachinesState::with_url("http://127.0.0.1:4317");
    state.remove_machine("lab-1");
    assert!(!state.busy);
    assert!(
        state.notice.contains("fora do snapshot"),
        "{}",
        state.notice
    );
}

#[test]
fn remove_machine_drains_worker_result_through_poll() {
    // Com a máquina no snapshot e autoridade inalcançável (loopback
    // fechado): o worker roda, o poll libera o busy e o aviso traz a
    // falha real — nunca sucesso fingido.
    let machine = MachineRecord {
        machine_id: String::from("lab-1"),
        host: String::from("192.168.1.61"),
        user: String::from("tese"),
        node_url: String::from("http://127.0.0.1:1"),
        services_hint: None,
        registered_at_ns: 1,
    };
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    state.snapshot = Some(Snapshot {
        items: vec![ItemView {
            id: DefinitionId(machine.catalog_id()),
            value: machine_catalog_value(&machine).expect("valor"),
            revision: Revision(7),
        }],
        cursor: Cursor(7),
    });
    state.remove_machine("lab-1");
    assert!(state.busy);
    for _ in 0..200 {
        if !state.busy {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.poll();
    }
    assert!(!state.busy);
    assert!(!state.notice.is_empty());
    assert!(!state.notice.contains("removida em"), "{}", state.notice);
}

#[test]
fn default_state_starts_with_empty_tokens_and_unknown_probes() {
    // Arranque honesto: sem tokens em memória, nenhuma sonda feita.
    let state = MachinesState::with_url("http://127.0.0.1:4317");
    assert!(state.tokens.is_empty());
    assert!(state.probes.is_empty());
    assert!(state.machines().is_empty());
    assert!(state.missing_token_urls().is_empty());
    assert_eq!(ProbeState::Unknown.dot(), "?");
}

#[test]
fn probe_all_ignores_empty_and_counts_pending() {
    // Given: painel ocioso.
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    // When: lista vazia.
    state.probe_all(&[]);
    // Then: nada acontece.
    assert!(!state.busy);
    assert_eq!(state.pending_probes, 0);
    // When: duas URLs (loopback fechado = falha rápida determinística).
    state.probe_all(&[
        String::from("http://127.0.0.1:1"),
        String::from("http://127.0.0.1:2"),
    ]);
    // Then: ocupado com 2 sondas em voo.
    assert!(state.busy);
    assert_eq!(state.pending_probes, 2);
}

#[test]
fn poll_releases_busy_only_on_last_probe() {
    // Given: 2 sondas em voo com canal injetado (sem rede).
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    let (tx, rx) = mpsc::channel();
    state.receiver = Some(rx);
    state.busy = true;
    state.pending_probes = 2;
    let probed = |url: &str| MachinesMsg::Probed {
        node_url: String::from(url),
        status: ProbeStatus {
            state: ProbeState::Offline,
            detail: String::from("fechado"),
        },
    };
    // When: primeira resposta.
    tx.send(probed("http://a:4317")).expect("canal vivo");
    state.poll();
    // Then: ainda ocupado (falta 1).
    assert_eq!(state.pending_probes, 1);
    assert!(state.busy);
    assert!(state.receiver.is_some());
    // When: última resposta.
    tx.send(probed("http://b:4317")).expect("canal vivo");
    state.poll();
    // Then: liberado com as 2 sondas aplicadas.
    assert_eq!(state.pending_probes, 0);
    assert!(!state.busy);
    assert_eq!(state.probes.len(), 2);
}

#[test]
fn test_and_register_requires_identity() {
    // Given: formulário vazio.
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    // When: Testar & Registrar sem identidade.
    state.test_and_register();
    // Then: aviso, sem sonda.
    assert!(!state.busy);
    assert!(state.notice.contains("obrigatórios"));
}

#[test]
fn publish_after_probe_chains_only_on_online() {
    // Given: formulário válido + probe em voo com publicação pendente.
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    state.form_machine_id = String::from("novo-70");
    state.form_node_url = String::from("http://127.0.0.1:1");
    let (tx, rx) = mpsc::channel();
    state.receiver = Some(rx);
    state.busy = true;
    state.pending_probes = 1;
    state.pending_publish_after_probe = true;
    // When: probe volta Online.
    tx.send(MachinesMsg::Probed {
        node_url: String::from("http://127.0.0.1:1"),
        status: ProbeStatus {
            state: ProbeState::Online,
            detail: String::from("protocolo 1.0"),
        },
    })
    .expect("canal vivo");
    state.poll();
    // Then: publicação encadeada (busy rearmado pelo publish).
    assert!(state.busy);
    assert!(state.receiver.is_some());
    assert!(!state.pending_publish_after_probe);

    // Given: mesma pendência, outro estado.
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    state.form_machine_id = String::from("novo-70");
    state.form_node_url = String::from("http://127.0.0.1:1");
    let (tx, rx) = mpsc::channel();
    state.receiver = Some(rx);
    state.busy = true;
    state.pending_probes = 1;
    state.pending_publish_after_probe = true;
    // When: probe volta Offline.
    tx.send(MachinesMsg::Probed {
        node_url: String::from("http://127.0.0.1:1"),
        status: ProbeStatus {
            state: ProbeState::Offline,
            detail: String::from("fechado"),
        },
    })
    .expect("canal vivo");
    state.poll();
    // Then: cancelado com aviso, sem publicar.
    assert!(!state.busy);
    assert!(state.notice.contains("NÃO persistido"));
}

#[test]
fn clear_form_empties_manual_fields() {
    // Given: formulário preenchido.
    let mut state = MachinesState::with_url("http://127.0.0.1:1");
    state.form_machine_id = String::from("x");
    state.form_node_url = String::from("http://x:4317");
    state.form_token = String::from("segredo");
    // When: Limpar.
    state.clear_form();
    // Then: os 3 campos do design vazios.
    assert!(state.form_machine_id.is_empty());
    assert!(state.form_node_url.is_empty());
    assert!(state.form_token.is_empty());
}

#[test]
fn host_from_url_parses_authority() {
    assert_eq!(host_from_url("http://192.168.1.70:4317"), "192.168.1.70");
    assert_eq!(host_from_url("http://lab-01:4317/rpc"), "lab-01");
    assert_eq!(host_from_url("http://lab-01"), "lab-01");
    assert_eq!(host_from_url("http://user@h:4317/x?y=1"), "h");
    assert_eq!(host_from_url("http://[fe80::1]:4317"), "fe80::1");
}

#[test]
fn auth_prefers_field_then_memory_token() {
    // Given: autoridade sem campo explícito, com token em memória.
    // (host `.invalid`: nunca tem token em disco — RFC 2606.)
    let mut state = MachinesState::with_url("http://no-existe.invalid:4317");
    state.tokens.insert(
        String::from("http://no-existe.invalid:4317"),
        String::from("mem"),
    );
    // Then: usa o da memória.
    assert_eq!(state.auth().as_deref(), Some("mem"));
    // When: campo explícito preenchido.
    state.authority_token = String::from("campo");
    // Then: o campo vence.
    assert_eq!(state.auth().as_deref(), Some("campo"));
    // When: ambos vazios.
    state.authority_token.clear();
    state.tokens.clear();
    // Then: sem auth.
    assert_eq!(state.auth(), None);
}
