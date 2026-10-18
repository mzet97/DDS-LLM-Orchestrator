use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn session_with_identity() -> (SshSession, PathBuf) {
    let dir = std::env::temp_dir().join(format!("studio-ssh-session-{}", std::process::id()));
    let identity_dir = dir.join("id");
    std::fs::create_dir_all(&identity_dir).expect("mkdir");
    std::fs::write(identity_dir.join("studio_ed25519"), b"pem-falso").expect("pem");
    let mut session = SshSession::with_paths(identity_dir, dir.join("trust.json"));
    session.host = String::from("127.0.0.1");
    session.port = String::from("2222");
    session.username = String::from("tester");
    (session, dir)
}

fn drain(session: &mut SshSession) {
    for _ in 0..200 {
        if !session.busy() && session.phase != SshPhase::Connecting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        session.poll();
    }
    session.poll();
}

#[test]
fn phase_idle_to_done_with_output() {
    let (mut session, _dir) = session_with_identity();
    session.start_with(|_t, _p, _s, _c, _cmd| Ok(String::from("saida-remota")));
    assert!(session.busy());
    drain(&mut session);
    assert_eq!(session.phase, SshPhase::Done);
    assert_eq!(session.output, "saida-remota");
}

#[test]
fn unknown_host_blocks_then_approve_persists_and_reconnects() {
    let (mut session, dir) = session_with_identity();
    session.start_with(|_t, _p, _s, _c, _cmd| {
        Err(BridgeError::UnknownHost {
            host: String::from("127.0.0.1"),
            port: 2222,
            key_type: String::from("ssh-ed25519"),
            fingerprint: String::from("SHA256:impressao-real"),
        })
    });
    drain(&mut session);
    assert_eq!(
        session.phase,
        SshPhase::AwaitingApproval {
            key_type: String::from("ssh-ed25519"),
            fingerprint: String::from("SHA256:impressao-real"),
        }
    );
    let approved = session
        .approve_displayed_with(|_t, _p, _s, _c, _cmd| Ok(String::from("PROVA_OK")));
    assert!(approved);
    drain(&mut session);
    assert_eq!(session.phase, SshPhase::Done);
    assert_eq!(session.output, "PROVA_OK");
    // Aprovação persistida sobrevive à reabertura do cofre.
    let file = TrustFile::open(&dir.join("trust.json")).expect("cofre");
    assert_eq!(file.store().approvals().len(), 1);
    assert_eq!(
        file.store().approvals()[0].key.fingerprint,
        "SHA256:impressao-real"
    );
}

#[test]
fn changed_key_blocks_without_approval() {
    let (mut session, _dir) = session_with_identity();
    session.start_with(|_t, _p, _s, _c, _cmd| {
        Err(BridgeError::ChangedKey {
            host: String::from("127.0.0.1"),
            port: 2222,
            known: String::from("SHA256:antiga"),
            presented: String::from("SHA256:nova"),
        })
    });
    drain(&mut session);
    assert_eq!(session.phase, SshPhase::Failed);
    assert!(session.notice.contains("mudou a chave"), "{}", session.notice);
    // Sem aprovação inline para chave alterada.
    assert!(!session.approve_displayed_with(|_t, _p, _s, _c, _cmd| Ok(String::new())));
}

#[test]
fn missing_identity_fails_before_any_worker() {
    let dir = std::env::temp_dir().join(format!("studio-ssh-sem-id-{}", std::process::id()));
    let mut session = SshSession::with_paths(dir.join("id"), dir.join("trust.json"));
    session.host = String::from("127.0.0.1");
    session.username = String::from("tester");
    let called = Arc::new(AtomicBool::new(false));
    let flag = called.clone();
    session.start_with(move |_t, _p, _s, _c, _cmd| {
        flag.store(true, Ordering::SeqCst);
        Ok(String::new())
    });
    assert_eq!(session.phase, SshPhase::Failed);
    assert!(session.notice.contains("identidade dedicada ausente"));
    assert!(!called.load(Ordering::SeqCst));
}
