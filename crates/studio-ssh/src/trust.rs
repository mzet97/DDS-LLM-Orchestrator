//! Confiança de hosts para a bridge SSH (G-02; lógica pura + persistência).
//!
//! Chave alterada BLOQUEIA; host desconhecido exige aprovação explícita.
//! A aprovação registra contexto (projeto, alias, host, porta, algoritmo,
//! impressão), quem aprovou, quando e qual chave foi substituída. A
//! verificação usa a chave **efetivamente apresentada no handshake**
//! (via [`crate::bridge`]), nunca strings da própria configuração.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Algoritmo + impressão no formato OpenSSH (`SHA256:…`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyIdentity {
    pub key_type: String,
    pub fingerprint: String,
}

/// Aprovação registrada por um operador para um host:porta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub project: String,
    pub alias: String,
    pub host: String,
    pub port: u16,
    pub key: KeyIdentity,
    pub approved_by: String,
    pub approved_at_unix: u64,
    /// Impressão anterior substituída por esta aprovação, se houve.
    pub replaced: Option<KeyIdentity>,
}

/// Erros do cofre (decisões de confiança, sem I/O).
#[derive(Debug, Error)]
pub enum TrustError {
    /// Host:porta novo: nada aprovado — exige decisão explícita.
    #[error("host {host}:{port} desconhecido: confira a impressao {fingerprint} por fonte independente antes de aprovar")]
    Unknown {
        host: String,
        port: u16,
        fingerprint: String,
    },
    /// Chave alterada: bloqueia até decisão explícita; sem auto-aceite.
    #[error("host {host}:{port} mudou a chave (era {known}, veio {presented}): conexao bloqueada")]
    Changed {
        host: String,
        port: u16,
        known: String,
        presented: String,
    },
    /// Arquivo de confiança ilegível ou corrompido.
    #[error("arquivo de confianca ilegivel em {path}: {detail}")]
    Unreadable { path: String, detail: String },
}

/// Cofre em memória: aprovações por (host, porta). Persistência em
/// [`TrustFile`]; o handshake usa [`crate::bridge`] com a chave real.
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    approvals: Vec<Approval>,
}

impl TrustStore {
    /// Cofre vazio: nenhum host confiável.
    #[must_use]
    pub fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Cofre a partir de aprovações já persistidas.
    #[must_use]
    pub fn from_approvals(approvals: Vec<Approval>) -> Self {
        Self { approvals }
    }

    /// Aprovações atuais (para persistir).
    #[must_use]
    pub fn approvals(&self) -> &[Approval] {
        &self.approvals
    }

    /// Verifica a chave apresentada no handshake contra a aprovação.
    pub fn check(&self, host: &str, port: u16, presented: &KeyIdentity) -> Result<(), TrustError> {
        match self
            .approvals
            .iter()
            .find(|item| item.host == host && item.port == port)
        {
            None => Err(TrustError::Unknown {
                host: String::from(host),
                port,
                fingerprint: presented.fingerprint.clone(),
            }),
            Some(approval) if approval.key == *presented => Ok(()),
            Some(approval) => Err(TrustError::Changed {
                host: String::from(host),
                port,
                known: approval.key.fingerprint.clone(),
                presented: presented.fingerprint.clone(),
            }),
        }
    }

    /// Aprovação explícita do operador; registra o contexto e a chave
    /// substituída (quando houver). Nunca chamada sem decisão humana.
    pub fn approve(&mut self, approval: Approval) {
        let replaced = self
            .approvals
            .iter()
            .find(|item| item.host == approval.host && item.port == approval.port)
            .map(|item| item.key.clone());
        let mut approval = approval;
        if approval.replaced.is_none() {
            approval.replaced = replaced;
        }
        self.approvals
            .retain(|item| !(item.host == approval.host && item.port == approval.port));
        self.approvals.push(approval);
    }
}

/// Arquivo JSON de aprovações por projeto/instalação (ex.:
/// `~/.config/studio/trust/<projeto>.json`, modo 0600). Chave privada
/// NUNCA entra aqui — só impressões públicas.
#[derive(Debug)]
pub struct TrustFile {
    path: std::path::PathBuf,
    store: TrustStore,
}

impl TrustFile {
    /// Abre ou cria vazio; corrompido é erro, nunca silêncio.
    pub fn open(path: &std::path::Path) -> Result<Self, TrustError> {
        let fail = |detail: String| TrustError::Unreadable {
            path: path.display().to_string(),
            detail,
        };
        let store = match std::fs::read(path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => TrustStore::new(),
            Err(err) => return Err(fail(err.to_string())),
            Ok(bytes) => {
                let approvals: Vec<Approval> =
                    serde_json::from_slice(&bytes).map_err(|err| fail(err.to_string()))?;
                TrustStore::from_approvals(approvals)
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            store,
        })
    }

    /// Cofre vivo (leitura do handshake).
    #[must_use]
    pub fn store(&self) -> &TrustStore {
        &self.store
    }

    /// Cofre vivo (aprovação explícita do operador).
    pub fn store_mut(&mut self) -> &mut TrustStore {
        &mut self.store
    }

    /// Persiste as aprovações (0600 em Unix).
    pub fn save(&self) -> Result<(), TrustError> {
        let fail = |detail: String| TrustError::Unreadable {
            path: self.path.display().to_string(),
            detail,
        };
        let bytes = serde_json::to_vec_pretty(self.store.approvals())
            .map_err(|err| fail(err.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)
                .map_err(|err| fail(err.to_string()))?;
            use std::io::Write;
            file.write_all(&bytes)
                .map_err(|err| fail(err.to_string()))?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&self.path, bytes).map_err(|err| fail(err.to_string()))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
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
}
