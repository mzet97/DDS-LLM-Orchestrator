use super::*;

fn key(fingerprint: &str) -> KeyIdentity {
    KeyIdentity {
        key_type: String::from("ssh-ed25519"),
        fingerprint: String::from(fingerprint),
    }
}

fn approval(fingerprint: &str) -> Approval {
    Approval {
        project: String::from("tese"),
        alias: String::from("gpu61"),
        host: String::from("192.168.1.61"),
        port: 22,
        key: key(fingerprint),
        approved_by: String::from("operador"),
        approved_at_unix: 1_700_000_000,
        replaced: None,
    }
}

#[test]
fn unknown_host_port_requires_approval() {
    let store = TrustStore::new();

    let err = store
        .check("192.168.1.61", 22, &key("SHA256:AAA"))
        .expect_err("novo bloqueia");

    assert!(matches!(err, TrustError::Unknown { .. }));
}

#[test]
fn same_port_different_host_is_not_trusted() {
    let mut store = TrustStore::new();
    store.approve(approval("SHA256:AAA"));

    let err = store
        .check("192.168.1.62", 22, &key("SHA256:AAA"))
        .expect_err("outro host");

    assert!(matches!(err, TrustError::Unknown { .. }));
}

#[test]
fn changed_key_blocks_and_records_replacement() {
    let mut store = TrustStore::new();
    store.approve(approval("SHA256:AAA"));

    let err = store
        .check("192.168.1.61", 22, &key("SHA256:BBB"))
        .expect_err("mudanca");
    assert!(matches!(err, TrustError::Changed { .. }));

    let mut second = approval("SHA256:BBB");
    second.approved_at_unix = 1_700_000_001;
    store.approve(second);
    let current = store
        .approvals()
        .iter()
        .find(|item| item.host == "192.168.1.61")
        .expect("aprovado");
    assert_eq!(current.replaced, Some(key("SHA256:AAA")));
    assert!(store.check("192.168.1.61", 22, &key("SHA256:BBB")).is_ok());
}

#[test]
fn file_roundtrip_survives_restart() {
    let dir = std::env::temp_dir().join(format!("studio-trust-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp");
    let path = dir.join("tese.json");

    let mut file = TrustFile::open(&path).expect("cria vazio");
    file.store_mut().approve(approval("SHA256:AAA"));
    file.save().expect("salva");

    let reopened = TrustFile::open(&path).expect("reabre");
    assert!(reopened
        .store()
        .check("192.168.1.61", 22, &key("SHA256:AAA"))
        .is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
