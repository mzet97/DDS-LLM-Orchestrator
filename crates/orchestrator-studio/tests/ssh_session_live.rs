//! Sessão real da GUI contra servidor SSH descartável em processo.
//!
//! Evidência nova (não repete T-800-28/29): o caminho REAL da GUI
//! (`SshSession::start` + `poll` + `approve_displayed`) executa handshake
//! SSH de verdade em loopback — desconhecido bloqueia com a impressão
//! REAL, aprovação persiste no cofre e a segunda conexão entrega a saída.
//! Nenhuma VM, nenhuma malha operacional, nenhuma UI aberta.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use orchestrator_studio::ssh_session::{SshPhase, SshSession, SshSessionConfig};
use russh::server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId};
use studio_ssh::SshTarget;

#[derive(Clone)]
struct LiveServer {
    allowed: Arc<HashMap<String, String>>,
}

impl Server for LiveServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}

impl Handler for LiveServer {
    type Error = russh::Error;

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

struct Guard {
    address: std::net::SocketAddr,
    host_fingerprint: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn spawn_server(client_openssh: &str) -> Guard {
    use russh::keys::ssh_key::{Algorithm, PrivateKey};
    let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("host key");
    let host_fingerprint = host_key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let mut allowed = HashMap::new();
    allowed.insert(String::from("tester"), format!("tester:{client_openssh}"));
    let (tx_addr, rx_addr) = std::sync::mpsc::channel();
    let (tx_stop, rx_stop) = tokio::sync::oneshot::channel::<()>();
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime do servidor");
        runtime.block_on(async move {
            let config = Arc::new(russh::server::Config {
                keys: vec![host_key],
                ..Default::default()
            });
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            tx_addr.send(listener.local_addr().expect("addr")).expect("addr");
            let handler = LiveServer {
                allowed: Arc::new(allowed),
            };
            let mut rx_stop = rx_stop;
            loop {
                tokio::select! {
                    _ = &mut rx_stop => break,
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break };
                        let config = config.clone();
                        let mut handler = handler.clone();
                        tokio::spawn(async move {
                            let _ = russh::server::run_stream(config, stream, handler.new_client(None)).await;
                        });
                    }
                }
            }
        });
    });
    let address = rx_addr.recv_timeout(Duration::from_secs(10)).expect("addr");
    Guard {
        address,
        host_fingerprint,
        shutdown: Some(tx_stop),
        thread: Some(thread),
    }
}

/// Drena `poll` até a sessão sair de `Running` (sinal de conclusão real).
fn settle(session: &mut SshSession) {
    let start = Instant::now();
    while matches!(session.phase, SshPhase::Running) {
        session.poll();
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "sessão travou em Running"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn gui_session_approves_unknown_host_then_delivers_output() {
    // Given: identidade dedicada gerada + servidor descartável que só
    // aceita essa pública, com host key desconhecida do cofre.
    let dir = std::env::temp_dir().join(format!("ssh-live-{}", std::process::id()));
    let paths = studio_ssh::identity::generate(&dir, "senha-teste-123").expect("identidade");
    let public_line = std::fs::read_to_string(&paths.public_openssh).expect("pub");
    let key_part: String = public_line
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let server = spawn_server(&key_part);

    let mut session = SshSession::new();
    session.config = SshSessionConfig {
        project: String::from("teste"),
        alias: String::from("descartavel"),
        target: SshTarget {
            host: String::from("127.0.0.1"),
            port: server.address.port(),
            username: String::from("tester"),
        },
        command: String::from("prove"),
        identity_pem: paths.private_pem,
        trust_path: dir.join("cofre.json"),
        passphrase: String::from("senha-teste-123"),
    };

    // When: primeira conexão contra host desconhecido.
    session.start();
    settle(&mut session);

    // Then: bloqueia exibindo a impressão REAL do servidor.
    match &session.phase {
        SshPhase::NeedsApproval {
            key_type,
            fingerprint,
        } => {
            assert_eq!(key_type, "ssh-ed25519");
            assert_eq!(fingerprint, &server.host_fingerprint);
        }
        other => panic!("esperava NeedsApproval, obteve {other:?}"),
    }

    // When: operador aprova e reconecta (decisão explícita).
    session.approve_displayed("teste-live").expect("aprova");
    assert_eq!(session.phase, SshPhase::Idle);
    session.start();
    settle(&mut session);

    // Then: saída real do comando através do caminho da GUI.
    match &session.phase {
        SshPhase::Done { output } => assert_eq!(output, "PROVA_OK"),
        other => panic!("esperava Done, obteve {other:?}"),
    }
}
