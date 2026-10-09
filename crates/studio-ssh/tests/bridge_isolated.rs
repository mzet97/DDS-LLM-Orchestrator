//! Validação isolada da bridge: servidor SSH descartável em processo.
//!
//! Nenhuma VM, nenhuma malha operacional: loopback com host key gerada
//! por teste. Prova: bloqueio de desconhecido com a impressão REAL,
//! aprovação persistida que sobrevive a reabertura, bloqueio em rotação
//! e falha de auth sem fallback — e nenhuma tentativa de senha.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use russh::server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId};
use studio_ssh::{identity, Approval, BridgeError, KeyIdentity, SshTarget, TrustFile, TrustStore};

#[derive(Clone)]
struct ProbeServer {
    allowed: Arc<HashMap<String, String>>,
    password_attempts: Arc<Mutex<u32>>,
}

impl Server for ProbeServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}

impl Handler for ProbeServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        *self.password_attempts.lock().expect("contador") += 1;
        Ok(Auth::reject())
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        let presented = format!("{user}:{}", public_key.to_openssh().unwrap_or_default());
        if self
            .allowed
            .values()
            .any(|line| line.trim() == presented.trim())
        {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let reply = if data == b"prove" {
            "PROVA_OK"
        } else {
            "desconhecido"
        };
        session.data(channel, reply.as_bytes().to_vec())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

struct Fixture {
    address: std::net::SocketAddr,
    host_fingerprint: String,
    password_attempts: Arc<Mutex<u32>>,
    client_private: russh::keys::ssh_key::PrivateKey,
}

async fn spawn_server() -> Fixture {
    use russh::keys::ssh_key::{Algorithm, PrivateKey};
    let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("host key");
    let host_fingerprint = host_key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let client_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("client key");
    let password_attempts = Arc::new(Mutex::new(0u32));
    let mut allowed = HashMap::new();
    allowed.insert(
        String::from("tester"),
        format!(
            "tester:{}",
            client_key.public_key().to_openssh().expect("pub")
        ),
    );

    let config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("addr");
    let handler = ProbeServer {
        allowed: Arc::new(allowed),
        password_attempts: password_attempts.clone(),
    };
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let _ = russh::server::run_stream(config, stream, handler).await;
    });
    // Segunda conexão (pós-aprovação) usa o mesmo listener de uso único?
    // Não: cada teste sobe seu próprio servidor; rotação troca a chave.
    Fixture {
        address,
        host_fingerprint,
        password_attempts,
        client_private: client_key,
    }
}

fn target(fixture: &Fixture) -> SshTarget {
    SshTarget {
        host: String::from("127.0.0.1"),
        port: fixture.address.port(),
        username: String::from("tester"),
    }
}

fn approval_for(fixture: &Fixture, fingerprint: &str) -> Approval {
    Approval {
        project: String::from("teste"),
        alias: String::from("descartavel"),
        host: String::from("127.0.0.1"),
        port: fixture.address.port(),
        key: KeyIdentity {
            key_type: String::from("ssh-ed25519"),
            fingerprint: String::from(fingerprint),
        },
        approved_by: String::from("teste"),
        approved_at_unix: 1_700_000_000,
        replaced: None,
    }
}

#[tokio::test]
async fn unknown_host_blocked_with_real_fingerprint() {
    let fixture = spawn_server().await;
    let trust = TrustStore::new();

    let err = studio_ssh::run_command(
        &target(&fixture),
        fixture.client_private.clone(),
        &trust,
        "prove",
    )
    .await
    .expect_err("desconhecido bloqueia");

    match err {
        BridgeError::UnknownHost {
            fingerprint,
            key_type,
            ..
        } => {
            assert_eq!(fingerprint, fixture.host_fingerprint);
            assert_eq!(key_type, "ssh-ed25519");
        }
        other => panic!("erro errado: {other}"),
    }
}

#[tokio::test]
async fn approved_and_persisted_connects_across_reopen() {
    let fixture = spawn_server().await;
    let dir = std::env::temp_dir().join(format!("ssh-bridge-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp");
    let path = dir.join("trust.json");

    let mut file = TrustFile::open(&path).expect("abre");
    file.store_mut()
        .approve(approval_for(&fixture, &fixture.host_fingerprint));
    file.save().expect("salva");
    let reopened = TrustFile::open(&path).expect("reabre");

    let output = studio_ssh::run_command(
        &target(&fixture),
        fixture.client_private.clone(),
        reopened.store(),
        "prove",
    )
    .await
    .expect("aprovado conecta");
    assert_eq!(output, "PROVA_OK");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn rotated_key_blocked_without_silent_reaccept() {
    let first = spawn_server().await;
    let second = spawn_server().await;
    assert_ne!(first.host_fingerprint, second.host_fingerprint);
    let mut trust = TrustStore::new();
    // Aprova a PRIMEIRA chave na porta do SEGUNDO servidor: simula rotação.
    let mut approval = approval_for(&second, &first.host_fingerprint);
    approval.port = second.address.port();
    trust.approve(approval);

    let err = studio_ssh::run_command(
        &target(&second),
        second.client_private.clone(),
        &trust,
        "prove",
    )
    .await
    .expect_err("rotacao bloqueia");

    match err {
        BridgeError::ChangedKey {
            known, presented, ..
        } => {
            assert_eq!(known, first.host_fingerprint);
            assert_eq!(presented, second.host_fingerprint);
        }
        other => panic!("erro errado: {other}"),
    }
}

#[tokio::test]
async fn wrong_client_key_fails_without_password_fallback() {
    use russh::keys::ssh_key::{Algorithm, PrivateKey};
    let fixture = spawn_server().await;
    let mut trust = TrustStore::new();
    trust.approve(approval_for(&fixture, &fixture.host_fingerprint));
    let wrong = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("chave errada");

    let err = studio_ssh::run_command(&target(&fixture), wrong, &trust, "prove")
        .await
        .expect_err("chave errada recusa");

    assert!(matches!(err, BridgeError::AuthFailed { .. }));
    assert_eq!(*fixture.password_attempts.lock().expect("contador"), 0);
}

#[tokio::test]
async fn identity_files_never_carry_passphrase() {
    let dir = std::env::temp_dir().join(format!("ssh-id-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let paths = identity::generate(&dir, "senha-de-desbloqueio").expect("gera");

    let raw_private = std::fs::read_to_string(&paths.private_pem).expect("pem");
    assert!(!raw_private.contains("senha-de-desbloqueio"));
    let raw_public = std::fs::read_to_string(&paths.public_openssh).expect("pub");
    assert!(!raw_public.contains("senha-de-desbloqueio"));
    assert!(raw_private.contains("OPENSSH PRIVATE KEY"));
    let _ = std::fs::remove_dir_all(&dir);
}
