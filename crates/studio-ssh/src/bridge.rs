//! Bridge SSH do Studio (G-04): chave dedicada + confiança no handshake.
//!
//! Regras rígidas, sem exceção silenciosa:
//! - só autenticação `publickey` com a identidade dedicada desbloqueada;
//!   falha de auth é erro, nunca tentativa de outro método;
//! - a chave do servidor é a **efetivamente apresentada no handshake**
//!   (`check_server_key`); desconhecida exige aprovação, alterada bloqueia;
//! - encaminhamento de agente NUNCA é solicitado (nenhuma chamada);
//! - nenhuma invocação de `ssh`/`scp`/`sshpass`/shell: tudo via `russh`.
//!
//! O operador aprova a partir da impressão exibida (conferida por fonte
//! independente) e a aprovação persiste em [`crate::trust::TrustFile`].

use std::sync::{Arc, Mutex};

use russh::client::{self, Handler};
use russh::keys::{ssh_key, PublicKeyOrCertificate};
use russh::ChannelMsg;
use thiserror::Error;

use crate::identity::IdentityError;
use crate::trust::{KeyIdentity, TrustError, TrustFile, TrustStore};

/// Alvo administrativo já autorizado pelo operador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
}

/// Erros da bridge (transporte, confiança e auth, sem fallback).
#[derive(Debug, Error)]
pub enum BridgeError {
    /// Host:porta sem aprovação: informa a impressão REAL apresentada.
    #[error("host desconhecido {host}:{port}: confira {fingerprint} ({key_type}) por fonte independente e aprove antes")]
    UnknownHost {
        host: String,
        port: u16,
        key_type: String,
        fingerprint: String,
    },
    /// Chave apresentada difere da aprovada: bloqueado, sem re-tentativa.
    #[error("host {host}:{port} mudou a chave (era {known}, veio {presented}): conexao bloqueada")]
    ChangedKey {
        host: String,
        port: u16,
        known: String,
        presented: String,
    },
    /// Auth `publickey` recusada: credencial errada ou não cadastrada.
    /// Nenhum outro método é tentado.
    #[error("autenticacao publickey recusada por {username}@{host}:{port}")]
    AuthFailed {
        username: String,
        host: String,
        port: u16,
    },
    /// Transporte/handshake falhou antes de qualquer decisão.
    #[error("transporte SSH falhou para {host}:{port}: {detail}")]
    Transport {
        host: String,
        port: u16,
        detail: String,
    },
    /// Comando remoto terminou com status diferente de zero: informa
    /// status, stdout e stderr (decodificados com perdas para nunca
    /// mascarar a falha com erro de decodificação).
    #[error("comando remoto falhou em {host}:{port} (status {status}): {stderr}")]
    CommandFailed {
        host: String,
        port: u16,
        status: u32,
        stdout: String,
        stderr: String,
    },
    /// Comando remoto morto por sinal: informa sinal, stdout e stderr.
    #[error("comando remoto morto por {signal} em {host}:{port}: {stderr}")]
    CommandSignaled {
        host: String,
        port: u16,
        signal: String,
        stdout: String,
        stderr: String,
    },
    /// Identidade (desbloqueio).
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// Cofre de confiança.
    #[error(transparent)]
    Trust(#[from] TrustError),
}

/// Verificador do handshake: compara a chave REAL apresentada com a
/// aprovação e captura a apresentada para erro tipado.
pub(crate) struct Verifier {
    expected: Option<KeyIdentity>,
    captured: Arc<Mutex<Option<KeyIdentity>>>,
}

impl Handler for Verifier {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let presented = match key {
            PublicKeyOrCertificate::PublicKey { key: public, .. } => KeyIdentity {
                key_type: public.algorithm().to_string(),
                fingerprint: public.fingerprint(ssh_key::HashAlg::Sha256).to_string(),
            },
            // Certificados OpenSSH não são autoridade de confiança do
            // Studio: registrados como apresentados e rejeitados.
            PublicKeyOrCertificate::Certificate(_) => KeyIdentity {
                key_type: String::from("ssh-certificate"),
                fingerprint: String::from("rejeitado: certificados nao sao autoridade do Studio"),
            },
        };
        *self.captured.lock().unwrap_or_else(|p| p.into_inner()) = Some(presented.clone());
        Ok(self.expected.as_ref() == Some(&presented))
    }
}

/// Executa `command` no alvo via sessão SSH autenticada e devolve o stdout
/// (UTF-8 estrito) quando o status de saída é zero. Status diferente de
/// zero vira [`BridgeError::CommandFailed`] (com stdout/stderr) e morte
/// por sinal vira [`BridgeError::CommandSignaled`]; stderr chega em
/// `ExtendedData` com `ext == 1` (RFC 4254 §5.2). `trust` decide sobre a
/// chave do handshake; `private_key` é a identidade dedicada já
/// desbloqueada (senha fora daqui, nunca persistida).
mod bridge_run;

pub use bridge_run::{run_command, run_command_blocking};
