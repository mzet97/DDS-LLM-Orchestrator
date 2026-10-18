use super::{
    merge_presence, retain_fresh, token_for_url, DiscoveredNode, DiscoveryState, HEARTBEAT_LEASE_NS,
};

#[test]
fn merge_updates_existing_and_sorts_new_entries() {
    let mut nodes = Vec::new();
    assert!(
        merge_presence(&mut nodes, "b:1", "http://b:1", true, 100),
        "primeira inserção é nova"
    );
    assert!(!merge_presence(&mut nodes, "b:1", "http://b:1", true, 200));
    assert_eq!(nodes[0].last_seen_unix_ns, 200, "heartbeat atualiza");

    assert!(merge_presence(&mut nodes, "a:1", "http://a:1", false, 50));
    let ids: Vec<_> = nodes.iter().map(|n| n.node_id.as_str()).collect();
    assert_eq!(ids, vec!["a:1", "b:1"], "lista ordenada por node_id");
}

#[test]
fn alive_window_follows_lease_with_clock_tolerance() {
    let now = 1_000_000_000_000u64;
    let node = |last_seen: u64| DiscoveredNode {
        node_id: String::from("n"),
        url: String::from("http://n:4317"),
        token_required: true,
        last_seen_unix_ns: last_seen,
        probe: None,
    };
    assert!(node(now - 5_000_000_000).is_alive(now), "5 s = vivo");
    assert!(!node(now - 30_000_000_000).is_alive(now), "30 s = morto");
}

#[test]
fn token_for_url_reads_hosts_dir() {
    let home = std::env::temp_dir().join(format!(
        "studio-token-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let dir = home.join(".config/studio/hosts");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("192.168.1.64.token"), "segredo-123\n").expect("token");

    assert_eq!(
        token_for_url(
            "http://192.168.1.64:4317",
            Some(home.to_str().expect("path"))
        ),
        Some(String::from("segredo-123"))
    );
    assert_eq!(
        token_for_url("http://10.0.0.1:4317", Some(home.to_str().expect("path"))),
        None
    );
    assert_eq!(token_for_url("http://10.0.0.1:4317", None), None);
    std::fs::remove_dir_all(&home).ok();
}

fn node(id: &str, url: &str) -> DiscoveredNode {
    DiscoveredNode {
        node_id: String::from(id),
        url: String::from(url),
        token_required: false,
        last_seen_unix_ns: 1,
        probe: None,
    }
}

#[test]
fn selection_follows_node_identity_across_resorted_snapshots() {
    // P1: o worker ordena por `node_id` — um nó novo antes do
    // selecionado desloca o índice; `selected_url()` deve continuar
    // apontando para o MESMO nó, sem nenhum clique.
    let mut state = DiscoveryState::new_disabled(170);
    state.observe(vec![node("b:1", "http://b:4317")]);
    state.select(0);
    assert_eq!(state.selected_url().as_deref(), Some("http://b:4317"));

    state.observe(vec![
        node("a:1", "http://a:4317"),
        node("b:1", "http://b:4317"),
    ]);
    assert_eq!(state.selected, Some(1), "índice remapeado para b:1");
    assert_eq!(state.selected_url().as_deref(), Some("http://b:4317"));
}

#[test]
fn selection_clears_when_selected_node_vanishes() {
    // Nó sumido do snapshot → seleção limpa (a auto-seleção do
    // `main` escolhe outro online; índice obsoleto nunca retargeteia).
    let mut state = DiscoveryState::new_disabled(170);
    state.observe(vec![node("b:1", "http://b:4317")]);
    state.select(0);
    state.observe(vec![node("a:1", "http://a:4317")]);
    assert_eq!(state.selected, None);
    assert_eq!(state.selected_url(), None);
}

#[test]
fn server_prune_evicts_entries_past_heartbeat_lease() {
    // P2: `ServerStatus` não carrega relógio no fio — o lease conta do
    // recebimento local, igual ao dos agentes (sem isso, inferência
    // morta fica no inventário para sempre).
    let now = 1_000_000_000_000u64;
    let mut entries: std::collections::BTreeMap<String, (&str, u64)> =
        std::collections::BTreeMap::new();
    entries.insert(String::from("fresco"), ("s", now - 5_000_000_000));
    entries.insert(
        String::from("obsoleto"),
        ("s", now - HEARTBEAT_LEASE_NS - 1),
    );
    let evicted = retain_fresh(&mut entries, now);
    assert_eq!(evicted, 1);
    assert!(entries.contains_key("fresco"), "5 s fica");
    assert!(!entries.contains_key("obsoleto"), ">30 s sai");
}

#[test]
fn observe_replaces_snapshot_and_clears_scanning() {
    let mut state = DiscoveryState::new_disabled(170);
    state.scanning = true;
    state.observe(vec![DiscoveredNode {
        node_id: String::from("n:1"),
        url: String::from("http://n:4317"),
        token_required: false,
        last_seen_unix_ns: 1,
        probe: None,
    }]);
    assert!(!state.scanning);
    assert_eq!(state.nodes.len(), 1);
    assert_eq!(state.nodes[0].node_id, "n:1");
}
