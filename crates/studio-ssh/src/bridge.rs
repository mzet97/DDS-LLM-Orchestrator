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
use crate::trust::{KeyIdentity, TrustError, TrustStore};

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
    /// Identidade (desbloqueio).
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// Cofre de confiança.
    #[error(transparent)]
    Trust(#[from] TrustError),
}

/// Verificador do handshake: compara a chave REAL apresentada com a
/// aprovação e captura a apresentada para erro tipado.
struct Verifier {
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
        *self.captured.lock().expect("verificador") = Some(presented.clone());
        Ok(self.expected.as_ref() == Some(&presented))
    }
}

/// Executa `command` no alvo via sessão SSH autenticada e devolve o stdout.
/// `trust` decide sobre a chave do handshake; `private_key` é a identidade
/// dedicada já desbloqueada (senha fora daqui, nunca persistida).
pub async fn run_command(
    target: &SshTarget,
    private_key: ssh_key::PrivateKey,
    trust: &TrustStore,
    command: &str,
) -> Result<String, BridgeError> {
    let address = format!("{}:{}", target.host, target.port);
    let fail = |detail: String| BridgeError::Transport {
        host: target.host.clone(),
        port: target.port,
        detail,
    };
    let expected = trust
        .approvals()
        .iter()
        .find(|item| item.host == target.host && item.port == target.port)
        .map(|item| item.key.clone());
    let captured = Arc::new(Mutex::new(None));
    let verifier = Verifier {
        expected,
        captured: captured.clone(),
    };
    let config = Arc::new(client::Config::default());
    let mut session = russh::client::connect(config, address.as_str(), verifier)
        .await
        .map_err(|err| map_connect_error(target, &captured, trust, err))?;
    let key_with_alg = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(private_key), None);
    let authenticated = session
        .authenticate_publickey(&target.username, key_with_alg)
        .await
        .map_err(|err| fail(format!("auth: {err}")))?;
    if !authenticated.success() {
        let _ = session
            .disconnect(russh::Disconnect::ByApplication, "auth recusada", "")
            .await;
        return Err(BridgeError::AuthFailed {
            username: target.username.clone(),
            host: target.host.clone(),
            port: target.port,
        });
    }
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|err| fail(err.to_string()))?;
    channel
        .exec(true, command)
        .await
        .map_err(|err| fail(err.to_string()))?;
    let mut output = Vec::new();
    loop {
        match channel.wait().await {
            None | Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => break,
            Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
            Some(_) => {}
        }
    }
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "fim", "")
        .await;
    String::from_utf8(output).map_err(|err| fail(format!("saida nao-utf8: {err}")))
}

/// Traduz falha de handshake em erro tipado usando a chave capturada.
fn map_connect_error(
    target: &SshTarget,
    captured: &Arc<Mutex<Option<KeyIdentity>>>,
    trust: &TrustStore,
    err: russh::Error,
) -> BridgeError {
    let presented = captured.lock().expect("verificador").clone();
    match presented {
        Some(key) => {
            let known = trust
                .approvals()
                .iter()
                .find(|item| item.host == target.host && item.port == target.port);
            match known {
                None => BridgeError::UnknownHost {
                    host: target.host.clone(),
                    port: target.port,
                    key_type: key.key_type,
                    fingerprint: key.fingerprint,
                },
                Some(approval) => BridgeError::ChangedKey {
                    host: target.host.clone(),
                    port: target.port,
                    known: approval.key.fingerprint.clone(),
                    presented: key.fingerprint,
                },
            }
        }
        None => BridgeError::Transport {
            host: target.host.clone(),
            port: target.port,
            detail: err.to_string(),
        },
    }
}
