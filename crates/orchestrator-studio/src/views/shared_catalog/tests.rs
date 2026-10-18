use super::shared_helpers::now_ms;
use super::*;

#[test]
fn kind_of_splits_prefix() {
    assert_eq!(kind_of("machine:orch-62"), "machine");
    assert_eq!(kind_of("sem-prefixo"), "sem-prefixo");
}

#[test]
fn ago_scales_units() {
    let now = now_ms();
    assert_eq!(ago(now), "0s");
    assert_eq!(ago(now - 14_000), "14s");
    assert_eq!(ago(now - 180_000), "3m");
    assert_eq!(ago(now - 7_200_000), "2h");
    // Futuro (relógio torto) não quebra: fixa no agora.
    assert_eq!(ago(now + 60_000), "0s");
}

#[test]
fn row_visible_combines_kind_and_search() {
    assert!(row_visible("machine:a", "{}", "machine", None, ""));
    assert!(!row_visible("machine:a", "{}", "machine", Some("cfg"), ""));
    assert!(row_visible(
        "machine:a",
        "{\"x\":1}",
        "machine",
        None,
        "x\":1"
    ));
    assert!(row_visible("machine:a", "{}", "machine", None, ":a"));
    assert!(!row_visible("machine:a", "{}", "machine", None, "zzz"));
    assert!(!row_visible(
        "machine:a",
        "{\"x\":1}",
        "machine",
        Some("cfg"),
        "x\":1"
    ));
}

#[test]
fn conflict_covers_struct_and_legacy_text() {
    let mut shared = SharedCatalog::with_url("http://127.0.0.1:1");
    assert!(!conflict_active(&shared));
    shared.notice = String::from("base obsoleta; vigente: Some(3)");
    assert!(conflict_active(&shared));
    shared.notice.clear();
    shared.conflict = Some(crate::catalog_remote::ConflictInfo {
        attempted_base: Some(2),
        current: Some(3),
    });
    assert!(conflict_active(&shared));
}
