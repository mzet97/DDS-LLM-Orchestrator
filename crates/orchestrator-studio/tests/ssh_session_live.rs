//! T-800-32: sessão da GUI contra servidor SSH descartável REAL em
//! processo (russh, loopback). Prova o caminho completo do painel:
//! `SshSession::start` → desconhecido bloqueia com a impressão REAL do
//! handshake → `approve_displayed` persiste no cofre → reconexão
//! entrega `PROVA_OK`. Nenhuma VM, nenhuma malha operacional.

use std::sync::Arc;

use orchestrator_studio::ssh_session::{SshPhase, SshSession};
use russh::server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId};
use studio_ssh::TrustFile;

#[derive(Clone)]
struct DisposableServer {
    allowed_openssh: Arc<String>,
}

impl Server for DisposableServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}

impl Handler for DisposableServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::reject())
    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        let presented = public_key.to_openssh().unwrap_or_default();
        if presented.trim() == self.allowed_openssh.trim() {
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
        _data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, "PROVA_OK".as_bytes().to_vec())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

fn wait_phase(session: &mut SshSession, wanted: &str) {
    for _ in 0..600 {
        session.poll();
        let at = match &session.phase {
            SshPhase::AwaitingApproval { .. } => "awaiting",
            SshPhase::Done => "done",
            SshPhase::Failed => "failed",
            _ => "other",
        };
        if at == wanted {
            return;
        }
        if wanted != "failed" && at == "failed" {
            panic!("sessão falhou: {}", session.notice);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!(
        "timeout esperando fase {wanted}; notice: {}",
        session.notice
    );
}

#[test]
fn gui_session_against_disposable_ssh_server() {
    let dir = std::env::temp_dir().join(format!("studio-ssh-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let identity_dir = dir.join("id");
    let trust_path = dir.join("trust.json");

    // Identidade dedicada real (PEM cifrado + .pub) gerada pela GUI.
    let mut session = SshSession::with_paths(identity_dir.clone(), trust_path.clone());
    session.passphrase = String::from("senha-do-teste");
    session.generate_identity();
    assert!(session.identity_exists(), "{}", session.notice);
    let pub_line = std::fs::read_to_string(identity_dir.join("studio_ed25519.pub")).expect("pub");
    let allowed_openssh = pub_line
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");

    // Servidor descartável: host key própria, aceita só a .pub gerada.
    use russh::keys::ssh_key::{Algorithm, PrivateKey};
    let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("host key");
    let host_fingerprint = host_key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (addr_tx, addr_rx) = std::sync::mpsc::channel();
    let handler = DisposableServer {
        allowed_openssh: Arc::new(allowed_openssh),
    };
    runtime.spawn(async move {
        let config = Arc::new(russh::server::Config {
            keys: vec![host_key],
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        addr_tx
            .send(listener.local_addr().expect("addr"))
            .expect("tx");
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let config = config.clone();
            let handler = handler.clone();
            tokio::spawn(async move {
                let _ = russh::server::run_stream(config, stream, handler).await;
            });
        }
    });
    let address = addr_rx.recv().expect("endereço do servidor");

    // Conexão 1: host desconhecido bloqueia com a impressão REAL.
    session.host = String::from("127.0.0.1");
    session.port = address.port().to_string();
    session.username = String::from("tester");
    session.command = String::from("prove");
    session.start();
    wait_phase(&mut session, "awaiting");
    match &session.phase {
        SshPhase::AwaitingApproval { fingerprint, .. } => {
            assert_eq!(fingerprint, &host_fingerprint);
        }
        other => panic!("fase inesperada: {other:?}"),
    }

    // Aprovação persiste no cofre e a reconexão entrega a saída real.
    assert!(session.approve_displayed());
    wait_phase(&mut session, "done");
    assert!(session.output.contains("PROVA_OK"), "{}", session.output);

    let file = TrustFile::open(&trust_path).expect("cofre reaberto");
    assert_eq!(file.store().approvals().len(), 1);
    assert_eq!(file.store().approvals()[0].host, "127.0.0.1");
    assert_eq!(
        file.store().approvals()[0].key.fingerprint,
        host_fingerprint
    );

    let _ = std::fs::remove_dir_all(&dir);
}
