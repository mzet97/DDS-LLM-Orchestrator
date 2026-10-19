//! Sessão SSH dedicada da GUI (T-800-29, G-04): estado do painel
//! "SSH Dedicado" sobre a bridge [`studio_ssh`] (russh puro, sem shell).
//!
//! Fluxo de fases: `Idle → Connecting → Done/Failed`, com parada em
//! `AwaitingApproval` quando o host é desconhecido — a impressão REAL
//! do handshake fica exibida e só a aprovação explícita do operador
//! (persistida no cofre) libera a reconexão. Chave alterada bloqueia
//! (`Failed`), sem opção de aprovação inline. A senha da identidade
//! vive só em memória, nunca em disco nem em log (RNF-04).
//!
//! Rede só em thread de trabalho + `poll` por frame (REQ/T-820-19).

use std::path::PathBuf;
use std::sync::mpsc;

use studio_ssh::{Approval, BridgeError, KeyIdentity, SshTarget, TrustFile};

/// Executor de um comando via bridge (injetável nos testes).
pub type SshRunner =
    dyn FnOnce(SshTarget, PathBuf, String, PathBuf, String) -> Result<String, BridgeError> + Send;

/// Fase visível da sessão (o painel renderiza a partir daqui).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshPhase {
    /// Pronta para conectar.
    Idle,
    /// Worker em voo (`poll` drena).
    Connecting,
    /// Host desconhecido: impressão real aguardando aprovação.
    AwaitingApproval {
        key_type: String,
        fingerprint: String,
    },
    /// Último comando concluiu; saída em `output`.
    Done,
    /// Falha tipada em `notice` (transporte, auth, chave alterada…).
    Failed,
}

enum SshMsg {
    Finished(Result<String, BridgeError>),
}

/// Estado do painel SSH: formulário + sessão + cofre.
pub struct SshSession {
    pub host: String,
    pub port: String,
    pub username: String,
    pub alias: String,
    pub project: String,
    pub operator: String,
    /// Senha da identidade dedicada — só memória (RNF-04).
    pub passphrase: String,
    pub command: String,
    pub phase: SshPhase,
    pub output: String,
    pub notice: String,
    /// Diretório da identidade dedicada (`studio_ed25519{,.pub}`).
    pub identity_dir: PathBuf,
    /// Arquivo JSON do cofre de confiança.
    pub trust_path: PathBuf,
    receiver: Option<mpsc::Receiver<SshMsg>>,
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Diretório padrão da identidade: `~/.config/studio/ssh`.
#[must_use]
pub fn default_identity_dir(home: Option<&str>) -> PathBuf {
    home.map_or_else(
        || PathBuf::from(".config/studio/ssh"),
        |home| PathBuf::from(home).join(".config/studio/ssh"),
    )
}

/// Cofre padrão: `~/.config/studio/trust.json`.
#[must_use]
pub fn default_trust_path(home: Option<&str>) -> PathBuf {
    home.map_or_else(
        || PathBuf::from(".config/studio/trust.json"),
        |home| PathBuf::from(home).join(".config/studio/trust.json"),
    )
}

impl Default for SshSession {
    fn default() -> Self {
        Self::new()
    }
}

impl SshSession {
    #[must_use]
    pub fn new() -> Self {
        let home = std::env::var("HOME").ok();
        Self::with_paths(
            default_identity_dir(home.as_deref()),
            default_trust_path(home.as_deref()),
        )
    }

    #[must_use]
    pub fn with_paths(identity_dir: PathBuf, trust_path: PathBuf) -> Self {
        Self {
            host: String::new(),
            port: String::from("22"),
            username: String::new(),
            alias: String::new(),
            project: String::from("studio"),
            operator: String::new(),
            passphrase: String::new(),
            command: String::from("hostname && uname -s"),
            phase: SshPhase::Idle,
            output: String::new(),
            notice: String::new(),
            identity_dir,
            trust_path,
            receiver: None,
        }
    }

    /// Caminho da chave privada dedicada.
    #[must_use]
    pub fn private_pem(&self) -> PathBuf {
        self.identity_dir.join("studio_ed25519")
    }

    /// Identidade já gerada nesta instalação.
    #[must_use]
    pub fn identity_exists(&self) -> bool {
        self.private_pem().is_file()
    }

    #[must_use]
    pub fn busy(&self) -> bool {
        self.phase == SshPhase::Connecting
    }

    /// Executa o comando do formulário com o executor real da bridge.
    pub fn start(&mut self) {
        self.start_with(|target, pem, passphrase, trust, command| {
            studio_ssh::run_command_blocking(&target, &pem, &passphrase, &trust, &command)
        });
    }

    /// Como [`start`](Self::start), com executor injetável (testes).
    pub fn start_with<R>(&mut self, runner: R)
    where
        R: FnOnce(SshTarget, PathBuf, String, PathBuf, String) -> Result<String, BridgeError>
            + Send
            + 'static,
    {
        if self.busy() {
            return;
        }
        let target = match self.target() {
            Ok(target) => target,
            Err(err) => {
                self.phase = SshPhase::Failed;
                self.notice = err;
                return;
            }
        };
        let pem = self.private_pem();
        if !pem.is_file() {
            self.phase = SshPhase::Failed;
            self.notice = format!(
                "identidade dedicada ausente em {} — gere a identidade antes de conectar",
                pem.display()
            );
            return;
        }
        let passphrase = self.passphrase.clone();
        let trust = self.trust_path.clone();
        let command = self.command.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = runner(target, pem, passphrase, trust, command);
            let _ = tx.send(SshMsg::Finished(result));
        });
        self.receiver = Some(rx);
        self.phase = SshPhase::Connecting;
        self.notice = String::from("conectando (handshake + publickey)…");
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let Some(rx) = &self.receiver else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.receiver = None;
        let SshMsg::Finished(result) = msg;
        match result {
            Ok(output) => {
                self.output = output;
                self.phase = SshPhase::Done;
                self.notice = String::from("comando concluído");
            }
            Err(BridgeError::UnknownHost {
                key_type,
                fingerprint,
                ..
            }) => {
                self.notice = format!(
                    "host desconhecido: confira a impressão {fingerprint} ({key_type}) por fonte independente antes de aprovar"
                );
                self.phase = SshPhase::AwaitingApproval {
                    key_type,
                    fingerprint,
                };
            }
            Err(err) => {
                self.phase = SshPhase::Failed;
                self.notice = err.to_string();
            }
        }
    }
}

mod ssh_approve;

#[cfg(test)]
mod tests;
