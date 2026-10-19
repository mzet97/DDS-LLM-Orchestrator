use super::*;

fn temp_journal(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "studio-catalog-{name}-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn stale_base_is_rejected_before_effects() {
    let mut authority = CatalogAuthority::new();
    let rev = authority
        .publish(String::from("d"), None, String::from("v1"), 0)
        .expect("cria");

    let err = authority
        .publish(String::from("d"), Some(rev + 9), String::from("v2"), 0)
        .expect_err("base obsoleta deve falhar");

    assert!(matches!(err, AuthorityError::Conflict { current: Some(_) }));
    assert_eq!(authority.snapshot().items.len(), 1);
}

#[test]
fn journal_rebuilds_across_restart() {
    let path = temp_journal("replay");
    let mut first = CatalogAuthority::with_journal(path.clone()).expect("boot novo");
    first
        .publish(String::from("d"), None, String::from("v1"), 0)
        .expect("publica");
    first
        .delete(String::from("d"), 0)
        .expect("exclui com base 0");

    let second = CatalogAuthority::with_journal(path.clone()).expect("reboot reconstroi");
    let _ = std::fs::remove_file(&path);

    assert!(second.snapshot().items.is_empty());
    let replayed = second.events_since(0).expect("cursor 0 valido");
    assert_eq!(replayed.len(), 2);
}

#[test]
fn corrupt_journal_fails_fast() {
    let path = temp_journal("corrompido");
    std::fs::write(&path, "não é json\n").expect("fixture");
    let err = CatalogAuthority::with_journal(path.clone()).expect_err("deve falhar");
    let _ = std::fs::remove_file(&path);

    assert!(matches!(err, AuthorityError::Corrupt { .. }));
}

/// REQ/T-820-18: falha de append desfaz a mutação em memória (replay do
/// journal) — memória e disco nunca divergem; a mutação não aceita pelo
/// disco não é reportada como aceita.
#[test]
fn append_failure_rolls_back_memory_to_journal_state() {
    use std::os::unix::fs::PermissionsExt;

    let path = temp_journal("rollback");
    let mut authority = CatalogAuthority::with_journal(path.clone()).expect("boot novo");
    authority
        .publish(String::from("d"), None, String::from("v1"), 0)
        .expect("primeira mutação journalizada");

    // Simula falha de disco: journal só-leitura (append falha, leitura ok).
    let mut perms = std::fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o444);
    std::fs::set_permissions(&path, perms).expect("chmod");

    let err = authority
        .publish(String::from("d2"), None, String::from("v2"), 0)
        .expect_err("append deve falhar");
    assert!(matches!(err, AuthorityError::Corrupt { .. }));

    // Rollback: memória == journal (d=v1 presente; d2 NÃO ficou aplicado).
    let snap = authority.snapshot();
    assert_eq!(snap.items.len(), 1, "apenas a mutação journalizada fica");
    assert_eq!(snap.items[0].id.0, "d");

    // Delete na mesma situação: mesmo contrato.
    let err = authority.delete(String::from("d"), 0).expect_err("falha");
    assert!(matches!(err, AuthorityError::Corrupt { .. }));
    assert_eq!(authority.snapshot().items.len(), 1, "rollback do delete");

    // Restaura permissões para limpar.
    let mut perms = std::fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o644);
    std::fs::set_permissions(&path, perms).expect("chmod");
    let _ = std::fs::remove_file(&path);
}

/// P2: sync falho após escrita bem-sucedida trunca a linha antes do
/// rebuild — a mutação reportada como falha não fica visível no journal
/// nem avança revisão no replay.
#[test]
fn failed_sync_truncates_line_so_replay_ignores_it() {
    use std::io::ErrorKind;

    let path = temp_journal("sync-falha");
    let mut authority = CatalogAuthority::with_journal(path.clone()).expect("boot novo");
    authority
        .publish(String::from("d"), None, String::from("v1"), 0)
        .expect("primeira mutacao journalizada");
    let before = std::fs::read(&path).expect("journal legivel");

    // Simula escrita ok + sync falho contra o arquivo real.
    let file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("abre append");
    let old_len = file.metadata().expect("meta").len();
    let err = CatalogAuthority::write_sync_truncate(
        &file,
        old_len,
        b"{\"op\":\"publish\",\"id\":\"d2\",\"base\":null,\"value\":\"v2\",\"generation\":0}\n",
        |_| Err(std::io::Error::other("fsync simulado")),
    )
    .expect_err("sync falho deve falhar");
    assert_eq!(err.kind(), ErrorKind::Other);
    drop(file);

    // Rollback físico: arquivo byte-idêntico ao anterior.
    let after = std::fs::read(&path).expect("journal legivel");
    assert_eq!(after, before, "linha falha nao pode ficar visivel");

    // Replay pós-falha: só a mutação aceita (revisão não avançou).
    let reboot = CatalogAuthority::with_journal(path.clone()).expect("reboot reconstroi");
    let snap = reboot.snapshot();
    assert_eq!(snap.items.len(), 1);
    assert_eq!(snap.items[0].id.0, "d");
    let events = reboot.events_since(0).expect("cursor 0 valido");
    assert_eq!(events.len(), 1);
    let _ = std::fs::remove_file(&path);
}
