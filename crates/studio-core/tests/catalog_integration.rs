//! Integração pública do catálogo (LOW da revisão 2026-10-18):
//! ciclo publish → update → events → delete → tombstone pela API pública.

use studio_core::catalog::{Catalog, CatalogError, Cursor, DefinitionId, EventKind};
use studio_core::revision::{Generation, Revision};

fn id(name: &str) -> DefinitionId {
    DefinitionId(String::from(name))
}

#[test]
fn publish_update_delete_cycle_via_public_api() {
    let mut catalog = Catalog::new();

    let created = catalog
        .publish(id("machine:a"), None, String::from("{}"), Generation(1))
        .expect("cria");
    assert_eq!(created, Revision(0));

    let updated = catalog
        .publish(
            id("machine:a"),
            Some(Revision(0)),
            String::from("{\"v\":2}"),
            Generation(2),
        )
        .expect("atualiza");
    assert_eq!(updated, Revision(1));

    let snapshot = catalog.snapshot();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].value, "{\"v\":2}");

    let events = catalog.events_since(Cursor(0)).expect("eventos");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, EventKind::Created);
    assert_eq!(events[1].kind, EventKind::Updated);

    let deleted = catalog
        .delete(&id("machine:a"), Revision(1))
        .expect("exclui");
    assert_eq!(deleted, Revision(2));
    assert!(catalog.snapshot().items.is_empty());

    // Tombstone impede republicar o mesmo id.
    let err = catalog
        .publish(id("machine:a"), None, String::from("{}"), Generation(3))
        .expect_err("tombstone");
    assert!(matches!(err, CatalogError::Tombstoned { .. }));
}

#[test]
fn conflict_and_expired_cursor_via_public_api() {
    let mut catalog = Catalog::new();
    catalog
        .publish(id("cfg:x"), None, String::from("v1"), Generation(1))
        .expect("cria");

    // Base obsoleta conflita.
    let err = catalog
        .publish(
            id("cfg:x"),
            Some(Revision(9)),
            String::from("v2"),
            Generation(2),
        )
        .expect_err("conflito");
    assert!(matches!(err, CatalogError::Conflict { .. }));

    // Cursor além do fim expira.
    assert!(matches!(
        catalog.events_since(Cursor(999)),
        Err(CatalogError::CursorExpired)
    ));
}
