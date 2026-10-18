use super::*;

#[test]
fn auth_curto_distinguishes_refused_from_missing() {
    assert_eq!(auth_curto("token recusado (401)"), "Token recusado");
    assert_eq!(
        auth_curto("token ausente (nó respondeu 401)"),
        "Token ausente"
    );
}

#[test]
fn port_of_parses_url_port() {
    assert_eq!(port_of("http://192.168.1.61:4317"), Some("4317"));
    assert_eq!(port_of("http://lab-01:4317/rpc"), Some("4317"));
    assert_eq!(port_of("http://lab-01"), None);
    assert_eq!(port_of("http://[fe80::1]:4317"), Some("4317"));
}

#[test]
fn truncate_keeps_short_and_ellipsizes_long() {
    assert_eq!(truncate("abc", 22), "abc");
    assert_eq!(truncate("studio-node-alpha-61-extra", 10), "studio-no…");
}

#[test]
fn heartbeat_label_thresholds_follow_lease() {
    assert_eq!(heartbeat_label(0).0, "● FRESCO");
    assert_eq!(heartbeat_label(6).0, "● VIVO");
    assert_eq!(heartbeat_label(11).0, "○ EXPIRANDO");
}

#[test]
fn age_frac_counts_seconds_from_last_seen() {
    let node = DiscoveredNode {
        node_id: String::from("n"),
        url: String::from("http://n:4317"),
        token_required: false,
        last_seen_unix_ns: 1_000_000_000,
        probe: None,
    };
    assert!((age_frac(&node, 6_500_000_000) - 5.5).abs() < 0.001);
}
