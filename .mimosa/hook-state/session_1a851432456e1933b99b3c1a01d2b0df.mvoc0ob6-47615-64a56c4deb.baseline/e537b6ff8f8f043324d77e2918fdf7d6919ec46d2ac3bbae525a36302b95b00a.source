//! Parte de `bridge` (extraída para manter os arquivos revisáveis).

use super::*;

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
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_status: Option<u32> = None;
    let mut exit_signal: Option<String> = None;
    loop {
        match channel.wait().await {
            None | Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => break,
            Some(ChannelMsg::Data { data }) => stdout.extend_from_slice(&data),
            Some(ChannelMsg::ExtendedData { data, ext }) => {
                if ext == 1 {
                    stderr.extend_from_slice(&data);
                }
            }
            Some(ChannelMsg::ExitStatus { exit_status: code }) => exit_status = Some(code),
            Some(ChannelMsg::ExitSignal { signal_name, .. }) => {
                exit_signal = Some(format!("{signal_name:?}"));
            }
            Some(_) => {}
        }
    }
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "fim", "")
        .await;
    if let Some(signal) = exit_signal {
        return Err(BridgeError::CommandSignaled {
            host: target.host.clone(),
            port: target.port,
            signal,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    if let Some(code) = exit_status.filter(|code| *code != 0) {
        return Err(BridgeError::CommandFailed {
            host: target.host.clone(),
            port: target.port,
            status: code,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    String::from_utf8(stdout).map_err(|err| fail(format!("saida nao-utf8: {err}")))
}

/// Caminho síncrono para a GUI (que não tem runtime async): desbloqueia a
/// identidade dedicada com a senha informada na hora, abre o cofre e
/// executa. Destinado a thread dedicada com `poll` — nunca na thread de UI.
pub fn run_command_blocking(
    target: &SshTarget,
    private_pem: &std::path::Path,
    passphrase: &str,
    trust_path: &std::path::Path,
    command: &str,
) -> Result<String, BridgeError> {
    let private_key = crate::identity::unlock(private_pem, passphrase)?;
    let trust_file = TrustFile::open(trust_path)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| BridgeError::Transport {
            host: target.host.clone(),
            port: target.port,
            detail: format!("runtime: {err}"),
        })?;
    runtime.block_on(run_command(
        target,
        private_key,
        trust_file.store(),
        command,
    ))
}

/// Traduz falha de handshake em erro tipado usando a chave capturada.
fn map_connect_error(
    target: &SshTarget,
    captured: &Arc<Mutex<Option<KeyIdentity>>>,
    trust: &TrustStore,
    err: russh::Error,
) -> BridgeError {
    let presented = captured.lock().unwrap_or_else(|p| p.into_inner()).clone();
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
