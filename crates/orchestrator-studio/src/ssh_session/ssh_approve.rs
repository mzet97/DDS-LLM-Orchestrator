//! Aprovação da impressão exibida (parte de `ssh_session`).

use super::*;

impl SshSession {
    /// Aprova a impressão exibida: persiste no cofre e reconecta com o
    /// executor real. Fora de `AwaitingApproval`, é no-op (`false`).
    pub fn approve_displayed(&mut self) -> bool {
        self.approve_displayed_with(|target, pem, passphrase, trust, command| {
            studio_ssh::run_command_blocking(&target, &pem, &passphrase, &trust, &command)
        })
    }

    /// Como [`approve_displayed`](Self::approve_displayed), injetável.
    pub fn approve_displayed_with<R>(&mut self, runner: R) -> bool
    where
        R: FnOnce(SshTarget, PathBuf, String, PathBuf, String) -> Result<String, BridgeError>
            + Send
            + 'static,
    {
        let SshPhase::AwaitingApproval {
            key_type,
            fingerprint,
        } = self.phase.clone()
        else {
            return false;
        };
        let target = match self.target() {
            Ok(target) => target,
            Err(err) => {
                self.phase = SshPhase::Failed;
                self.notice = err;
                return false;
            }
        };
        let approval = Approval {
            project: self.project.trim().to_string(),
            alias: if self.alias.trim().is_empty() {
                target.host.clone()
            } else {
                self.alias.trim().to_string()
            },
            host: target.host.clone(),
            port: target.port,
            key: KeyIdentity {
                key_type,
                fingerprint,
            },
            approved_by: if self.operator.trim().is_empty() {
                String::from("operador")
            } else {
                self.operator.trim().to_string()
            },
            approved_at_unix: now_unix_secs(),
            replaced: None,
        };
        let saved = TrustFile::open(&self.trust_path).and_then(|mut file| {
            file.store_mut().approve(approval);
            file.save()
        });
        match saved {
            Ok(()) => {
                self.notice = format!(
                    "impressão aprovada para {}:{} — reconectando",
                    target.host, target.port
                );
                self.phase = SshPhase::Idle;
                self.start_with(runner);
                true
            }
            Err(err) => {
                self.phase = SshPhase::Failed;
                self.notice = format!("falha ao persistir aprovação: {err}");
                false
            }
        }
    }

}

impl SshSession {
    /// Alvo validado do formulário (porta numérica, host/usuário dados).
    pub fn target(&self) -> Result<SshTarget, String> {
        let host = self.host.trim();
        let username = self.username.trim();
        if host.is_empty() || username.is_empty() {
            return Err(String::from("host e usuário são obrigatórios"));
        }
        let port: u16 = self
            .port
            .trim()
            .parse()
            .map_err(|_| String::from("porta inválida (esperado 1..=65535)"))?;
        Ok(SshTarget {
            host: String::from(host),
            port,
            username: String::from(username),
        })
    }

    /// Gera a identidade dedicada com a senha do formulário.
    pub fn generate_identity(&mut self) {
        match studio_ssh::identity::generate(&self.identity_dir, &self.passphrase) {
            Ok(paths) => {
                self.notice = format!(
                    "identidade gerada em {} — cadastre a .pub no host antes de conectar",
                    paths.public_openssh.display()
                );
            }
            Err(err) => {
                self.notice = format!("falha ao gerar identidade: {err}");
            }
        }
    }

    /// Aprovações persistidas no cofre (leitura; vazio se ilegível).
    #[must_use]
    pub fn approvals(&self) -> Vec<Approval> {
        TrustFile::open(&self.trust_path).map_or_else(
            |_| Vec::new(),
            |file| file.store().approvals().to_vec(),
        )
    }

}
