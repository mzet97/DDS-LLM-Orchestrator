//! Sessão SSH da GUI: desbloqueio + aprovação + execução sem travar a UI.
//!
//! A ponte real ([`studio_ssh`]) roda em thread dedicada com `poll` por
//! frame. Lógica de fases testável com runner injetado: `Idle` →
//! `Running` → `Done`/`Failed`, com desvio para `NeedsApproval` quando o
//! host é desconhecido. Aprovar persiste no cofre e volta a `Idle` —
//! reconectar é decisão explícita do operador, nunca automática.

use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use studio_ssh::{Approval, BridgeError, SshTarget};

/// Log de depuração do painel (ativar com `STUDIO_SSH_DEBUG=1`). Nunca
/// registra senha, conteúdo de chave nem a configuração completa.
pub(crate) fn debug_log(msg: &str) {
    if std::env::var_os("STUDIO_SSH_DEBUG").is_some() {
        eprintln!("[ssh-gui] {msg}");
    }
}

/// Nome curto da fase para log (sem conteúdo de saída nem configuração).
fn fase_nome(phase: &SshPhase) -> &'static str {
    match phase {
        SshPhase::Idle => "Idle",
        SshPhase::NeedsApproval { .. } => "NeedsApproval",
        SshPhase::Running => "Running",
        SshPhase::Done { .. } => "Done",
        SshPhase::Failed { .. } => "Failed",
    }
}

/// Alvo + credenciais de sessão (senha só em memória, nunca persistida).
#[derive(Debug, Clone)]
pub struct SshSessionConfig {
    pub project: String,
    pub alias: String,
    pub target: SshTarget,
    pub command: String,
    pub identity_pem: std::path::PathBuf,
    pub trust_path: std::path::PathBuf,
    pub passphrase: String,
}

/// Fases visíveis no painel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshPhase {
    Idle,
    NeedsApproval {
        key_type: String,
        fingerprint: String,
    },
    Running,
    Done {
        output: String,
    },
    Failed {
        error: String,
    },
}

enum SshMsg {
    Finished(Result<String, BridgeError>),
}

/// Estado da sessão SSH no painel.
pub struct SshSession {
    pub config: SshSessionConfig,
    pub phase: SshPhase,
    receiver: Option<mpsc::Receiver<SshMsg>>,
    repaint: Option<egui::Context>,
}

impl SshSession {
    /// Sessão fechada em `Idle`, sem efeito.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SshSessionConfig {
                project: String::from("tese"),
                alias: String::new(),
                target: SshTarget {
                    host: String::new(),
                    port: 22,
                    username: String::new(),
                },
                command: String::from("hostname"),
                identity_pem: std::path::PathBuf::new(),
                trust_path: std::path::PathBuf::new(),
                passphrase: String::new(),
            },
            phase: SshPhase::Idle,
            receiver: None,
            repaint: None,
        }
    }

    /// Registra o contexto egui para acordar a UI a partir da thread de
    /// conexão (egui::Context é Clone + Send + Sync em egui 0.36).
    pub fn set_repaint_source(&mut self, ctx: egui::Context) {
        self.repaint = Some(ctx);
    }

    /// Dispara a execução em thread; a UI segue livre (`poll` drena).
    pub fn start_with(
        &mut self,
        runner: impl FnOnce(SshSessionConfig) -> Result<String, BridgeError> + Send + 'static,
    ) {
        if matches!(self.phase, SshPhase::Running) {
            return;
        }
        let config = self.config.clone();
        let (tx, rx) = mpsc::channel();
        let repaint = self.repaint.clone();
        debug_log("start: thread de conexão disparada");
        std::thread::spawn(move || {
            let outcome = runner(config);
            debug_log("thread: conclusão recebida; acordando a UI");
            let _ = tx.send(SshMsg::Finished(outcome));
            if let Some(ctx) = repaint {
                ctx.request_repaint();
            }
        });
        self.receiver = Some(rx);
        self.phase = SshPhase::Running;
        debug_log("start: fase -> Running");
    }

    /// Caminho real: ponte síncrona da `studio_ssh` em thread dedicada.
    pub fn start(&mut self) {
        self.start_with(|config| {
            studio_ssh::run_command_blocking(
                &config.target,
                &config.identity_pem,
                &config.passphrase,
                &config.trust_path,
                &config.command,
            )
        });
    }

    /// Drena o resultado; chamar a cada frame enquanto `Running`.
    pub fn poll(&mut self) {
        let mut finished = None;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                let SshMsg::Finished(outcome) = msg;
                finished = Some(outcome);
            }
        }
        if let Some(outcome) = finished {
            self.receiver = None;
            let next = match outcome {
                Ok(output) => SshPhase::Done { output },
                Err(BridgeError::UnknownHost {
                    key_type,
                    fingerprint,
                    ..
                }) => SshPhase::NeedsApproval {
                    key_type,
                    fingerprint,
                },
                Err(err) => SshPhase::Failed {
                    error: err.to_string(),
                },
            };
            debug_log(&format!("poll: fase -> {}", fase_nome(&next)));
            self.phase = next;
            if let Some(ctx) = &self.repaint {
                ctx.request_repaint();
            }
        }
    }

    /// Aprovação explícita do operador sobre a impressão EXIBIDA (a ser
    /// conferida por fonte independente). Persiste e volta a `Idle`.
    pub fn approve_displayed(&mut self, approved_by: &str) -> Result<(), String> {
        debug_log("aprovar: handler disparado");
        let (key_type, fingerprint) = match &self.phase {
            SshPhase::NeedsApproval {
                key_type,
                fingerprint,
            } => (key_type.clone(), fingerprint.clone()),
            _ => {
                debug_log("aprovar: nada a aprovar na fase atual");
                return Err(String::from("nada a aprovar"));
            }
        };
        let mut file =
            studio_ssh::TrustFile::open(&self.config.trust_path).map_err(|err| {
                debug_log("aprovar: cofre falhou ao abrir");
                err.to_string()
            })?;
        debug_log("aprovar: cofre aberto");
        let replaced = file
            .store()
            .approvals()
            .iter()
            .find(|item| {
                item.host == self.config.target.host && item.port == self.config.target.port
            })
            .map(|item| item.key.clone());
        let mut approval = Approval {
            project: self.config.project.clone(),
            alias: self.config.alias.clone(),
            host: self.config.target.host.clone(),
            port: self.config.target.port,
            key: studio_ssh::KeyIdentity {
                key_type,
                fingerprint,
            },
            approved_by: String::from(approved_by),
            approved_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or(0),
            replaced: None,
        };
        if approval.replaced.is_none() {
            approval.replaced = replaced;
        }
        file.store_mut().approve(approval);
        file.save().map_err(|err| {
            debug_log("aprovar: cofre falhou ao persistir");
            err.to_string()
        })?;
        debug_log("aprovar: cofre persistido; fase -> Idle");
        self.phase = SshPhase::Idle;
        Ok(())
    }
}

impl Default for SshSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(session: &mut SshSession) {
        for _ in 0..1000 {
            session.poll();
            if !matches!(session.phase, SshPhase::Running) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn unknown_host_surfaces_fingerprint_for_approval() {
        let mut session = SshSession::new();
        session.start_with(|_| {
            Err(BridgeError::UnknownHost {
                host: String::from("h"),
                port: 22,
                key_type: String::from("ssh-ed25519"),
                fingerprint: String::from("SHA256:REAL"),
            })
        });
        drain(&mut session);

        assert_eq!(
            session.phase,
            SshPhase::NeedsApproval {
                key_type: String::from("ssh-ed25519"),
                fingerprint: String::from("SHA256:REAL"),
            }
        );
    }

    #[test]
    fn approval_persists_and_returns_to_idle() {
        let dir = std::env::temp_dir().join(format!("ssh-gui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp");
        let mut session = SshSession::new();
        session.config.trust_path = dir.join("trust.json");
        session.config.target.host = String::from("10.0.0.9");
        session.start_with(|_| {
            Err(BridgeError::UnknownHost {
                host: String::from("h"),
                port: 22,
                key_type: String::from("ssh-ed25519"),
                fingerprint: String::from("SHA256:REAL"),
            })
        });
        drain(&mut session);

        session.approve_displayed("operador-gui").expect("aprova");
        assert_eq!(session.phase, SshPhase::Idle);
        let file = studio_ssh::TrustFile::open(&dir.join("trust.json")).expect("reabre");
        assert!(file
            .store()
            .check(
                "10.0.0.9",
                22,
                &studio_ssh::KeyIdentity {
                    key_type: String::from("ssh-ed25519"),
                    fingerprint: String::from("SHA256:REAL"),
                }
            )
            .is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_key_and_auth_failure_are_terminal() {
        let mut session = SshSession::new();
        session.start_with(|_| {
            Err(BridgeError::ChangedKey {
                host: String::from("h"),
                port: 22,
                known: String::from("A"),
                presented: String::from("B"),
            })
        });
        drain(&mut session);
        assert!(matches!(session.phase, SshPhase::Failed { .. }));
        assert!(session.approve_displayed("x").is_err());

        let mut session = SshSession::new();
        session.start_with(|_| {
            Err(BridgeError::AuthFailed {
                username: String::from("u"),
                host: String::from("h"),
                port: 22,
            })
        });
        drain(&mut session);
        assert!(matches!(session.phase, SshPhase::Failed { .. }));
    }

    #[test]
    fn success_carries_output() {
        let mut session = SshSession::new();
        session.start_with(|_| Ok(String::from("maquina-61")));
        drain(&mut session);

        assert_eq!(
            session.phase,
            SshPhase::Done {
                output: String::from("maquina-61")
            }
        );
    }
}
