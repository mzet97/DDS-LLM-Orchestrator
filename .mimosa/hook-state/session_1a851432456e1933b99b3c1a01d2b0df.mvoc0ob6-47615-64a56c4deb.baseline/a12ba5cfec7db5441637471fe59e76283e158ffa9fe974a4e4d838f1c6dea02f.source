use super::*;
use crate::revision::Generation;

fn id(name: &str) -> DefinitionId {
    DefinitionId(name.into())
}

#[test]
fn stale_update_is_rejected_before_any_effect() {
    // Given: definição publicada na rev 0 (G-47).
    let mut catalog = Catalog::new();
    let rev = catalog
        .publish(id("agent-a"), None, "v1".into(), Generation(1))
        .unwrap();
    assert_eq!(rev, Revision(0));
    // When: segunda GUI tenta atualizar da base obsoleta... (rev 0 após
    // a primeira GUI já ter avançado para rev 1). Atualização aceita
    // carrega geração estritamente maior (REQ-801, T-830-03).
    catalog
        .publish(id("agent-a"), Some(Revision(0)), "v2".into(), Generation(2))
        .unwrap();
    let err = catalog
        .publish(
            id("agent-a"),
            Some(Revision(0)),
            "rival".into(),
            Generation(2),
        )
        .unwrap_err();
    // Then: conflito com a vigente e valor preservado.
    assert_eq!(
        err,
        CatalogError::Conflict {
            current: Some(Revision(1))
        }
    );
    let snap = catalog.snapshot();
    assert_eq!(snap.items[0].value, "v2");
}

#[test]
fn generation_regression_is_rejected_with_state_preserved() {
    // Given: item vigente com geração 2 (semente 1 + atualização 2).
    let mut catalog = Catalog::new();
    catalog
        .publish(id("dep"), None, "v1".into(), Generation(1))
        .unwrap();
    catalog
        .publish(id("dep"), Some(Revision(0)), "v2".into(), Generation(2))
        .unwrap();
    // When: GUI antiga tenta reimpor geração 1, regressiva (REQ-801).
    let err = catalog
        .publish(id("dep"), Some(Revision(1)), "antigo".into(), Generation(1))
        .unwrap_err();
    // Then: `StaleGeneration` tipado; repetida também é rejeitada.
    assert_eq!(
        err,
        CatalogError::StaleGeneration {
            got: Generation(1),
            current: Generation(2)
        }
    );
    assert_eq!(
        catalog
            .publish(id("dep"), Some(Revision(1)), "v2".into(), Generation(2))
            .unwrap_err(),
        CatalogError::StaleGeneration {
            got: Generation(2),
            current: Generation(2)
        }
    );
    // Estado preservado: valor e revisão da última aceita.
    let snap = catalog.snapshot();
    assert_eq!(snap.items[0].value, "v2");
    assert_eq!(snap.items[0].revision, Revision(1));
}

#[test]
fn monotonic_generation_is_accepted_and_advances_revision() {
    // Given/When: criação semeia a geração; atualizações só avançam.
    let mut catalog = Catalog::new();
    catalog
        .publish(id("dep"), None, "v1".into(), Generation(5))
        .unwrap();
    catalog
        .publish(id("dep"), Some(Revision(0)), "v2".into(), Generation(6))
        .unwrap();
    catalog
        .publish(id("dep"), Some(Revision(1)), "v3".into(), Generation(9))
        .unwrap();
    // Then: revisões avançam 0→1→2 e o valor vigente é o último.
    let snap = catalog.snapshot();
    assert_eq!(snap.items[0].value, "v3");
    assert_eq!(snap.items[0].revision, Revision(2));
}

#[test]
fn tombstone_blocks_resurrection_and_forces_new_identity() {
    // Given: item publicado e excluído (G-57).
    let mut catalog = Catalog::new();
    catalog
        .publish(id("tool-x"), None, "v1".into(), Generation(1))
        .unwrap();
    catalog.delete(&id("tool-x"), Revision(0)).unwrap();
    // When/Then: republicar o mesmo id é rejeitado...
    assert_eq!(
        catalog
            .publish(id("tool-x"), None, "v1".into(), Generation(2))
            .unwrap_err(),
        CatalogError::Tombstoned {
            deleted_at: Revision(1)
        }
    );
    // ...mas identidade nova com mesmo nome lógico é aceita.
    let rev = catalog
        .publish(id("tool-x#2"), None, "v1".into(), Generation(2))
        .unwrap();
    assert_eq!(rev, Revision(0));
}

#[test]
fn snapshot_and_events_form_gapless_sequence() {
    // Given: snapshot inicial (G-49).
    let mut catalog = Catalog::new();
    catalog
        .publish(id("a"), None, "1".into(), Generation(1))
        .unwrap();
    let snap = catalog.snapshot();
    // When: nova publicação após o snapshot.
    catalog
        .publish(id("b"), None, "2".into(), Generation(1))
        .unwrap();
    // Then: acompanhamento entrega exatamente o evento novo, contíguo.
    let events = catalog.events_since(snap.cursor).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Created);
    assert_eq!(events[0].id, id("b"));
    assert_eq!(events[0].seq + 1, catalog.snapshot().cursor.0);
}

#[test]
fn expired_cursor_forces_full_resync() {
    // Given: cursor antigo além da retenção (G-49).
    let mut catalog = Catalog::new();
    let old = catalog.snapshot().cursor;
    for i in 0..(RETENTION as u64 + 2) {
        catalog
            .publish(id(&format!("k{i}")), None, "v".into(), Generation(1))
            .unwrap();
    }
    // When/Then: acompanhar do cursor antigo exige ressincronização.
    assert_eq!(
        catalog.events_since(old).unwrap_err(),
        CatalogError::CursorExpired
    );
    // E o snapshot atual continua íntegro.
    assert_eq!(catalog.snapshot().items.len(), RETENTION + 2);
}
