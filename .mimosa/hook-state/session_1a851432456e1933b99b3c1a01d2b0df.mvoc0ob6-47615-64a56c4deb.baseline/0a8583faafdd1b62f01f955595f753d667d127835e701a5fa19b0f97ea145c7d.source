//! Identidade dedicada do Studio: Ed25519 por instalação (G-04).
//!
//! Uma chave por instalação do Studio, gerada pela biblioteca integrada
//! (sem `ssh-keygen`, sem shell). A privada fica cifrada com a senha de
//! desbloqueio (bcrypt-pbkdf + AES-256-CTR, formato OpenSSH) em arquivo
//! modo 0600 na máquina da instalação; a senha NUNCA é gravada. O
//! desbloqueio é explícito a cada carregamento. Só a pública (`.pub`,
//! linha `authorized_keys`) viaja no provisionamento — operação separada
//! que não altera nenhum host por si só.

use russh::keys::ssh_key;
use thiserror::Error;

/// Erros da identidade (geração e desbloqueio, sem rede).
#[derive(Debug, Error)]
pub enum IdentityError {
    /// Geração, cifra ou escrita falhou.
    #[error("falha na identidade em {path}: {detail}")]
    Storage { path: String, detail: String },
    /// Senha errada ou arquivo corrompido no desbloqueio.
    #[error("desbloqueio falhou em {path}: senha incorreta ou arquivo corrompido")]
    Unlock { path: String },
}

/// Caminhos da identidade gerada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityPaths {
    pub private_pem: std::path::PathBuf,
    pub public_openssh: std::path::PathBuf,
}

/// Gera Ed25519 dedicado em `dir` (`studio_ed25519` + `.pub`), privada
/// cifrada com `passphrase` e modo 0600. Diretório criado se ausente.
pub fn generate(dir: &std::path::Path, passphrase: &str) -> Result<IdentityPaths, IdentityError> {
    let fail = |detail: String| IdentityError::Storage {
        path: dir.display().to_string(),
        detail,
    };
    if passphrase.is_empty() {
        return Err(fail(String::from(
            "senha de desbloqueio vazia: recuse chaves sem senha",
        )));
    }
    std::fs::create_dir_all(dir).map_err(|err| fail(err.to_string()))?;
    let private = ssh_key::PrivateKey::random(&mut rand::rng(), ssh_key::Algorithm::Ed25519)
        .map_err(|err| fail(err.to_string()))?;
    let encrypted = private
        .encrypt(&mut rand::rng(), passphrase)
        .map_err(|err| fail(err.to_string()))?;
    let pem = encrypted
        .to_openssh(ssh_key::LineEnding::LF)
        .map_err(|err| fail(err.to_string()))?;
    let private_path = dir.join("studio_ed25519");
    write_private(&private_path, pem.as_str()).map_err(&fail)?;
    let public_line = format!(
        "{} studio-{}\n",
        private
            .public_key()
            .to_openssh()
            .map_err(|err| fail(err.to_string()))?,
        hostname_label()
    );
    let public_path = dir.join("studio_ed25519.pub");
    std::fs::write(&public_path, public_line).map_err(|err| fail(err.to_string()))?;
    Ok(IdentityPaths {
        private_pem: private_path,
        public_openssh: public_path,
    })
}

/// Desbloqueio explícito: lê a privada cifrada com a senha informada na
/// hora. A senha nunca é persistida por este módulo.
pub fn unlock(
    private_pem: &std::path::Path,
    passphrase: &str,
) -> Result<ssh_key::PrivateKey, IdentityError> {
    let text = std::fs::read_to_string(private_pem).map_err(|_| IdentityError::Unlock {
        path: private_pem.display().to_string(),
    })?;
    russh::keys::decode_secret_key(text.as_str(), Some(passphrase)).map_err(|_| {
        IdentityError::Unlock {
            path: private_pem.display().to_string(),
        }
    })
}

/// Impressão OpenSSH (`SHA256:…`) da pública correspondente.
pub fn public_fingerprint(
    private_pem: &std::path::Path,
    passphrase: &str,
) -> Result<String, IdentityError> {
    let private = unlock(private_pem, passphrase)?;
    Ok(private
        .public_key()
        .fingerprint(ssh_key::HashAlg::Sha256)
        .to_string())
}

fn hostname_label() -> String {
    String::from("studio")
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, pem: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| err.to_string())?;
    file.write_all(pem.as_bytes())
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, pem: &str) -> Result<(), String> {
    std::fs::write(path, pem).map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("studio-id-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn empty_passphrase_is_refused() {
        let dir = temp_dir("nosenha");

        let err = generate(&dir, "").expect_err("sem senha deve falhar");

        assert!(matches!(err, IdentityError::Storage { .. }));
    }

    #[test]
    fn generate_unlock_and_wrong_password() {
        let dir = temp_dir("ok");
        let paths = generate(&dir, "desbloqueio-explicito").expect("gera");

        let private = unlock(&paths.private_pem, "desbloqueio-explicito").expect("desbloqueia");
        assert_eq!(private.algorithm(), ssh_key::Algorithm::Ed25519);
        let fingerprint =
            public_fingerprint(&paths.private_pem, "desbloqueio-explicito").expect("impressao");
        assert!(fingerprint.starts_with("SHA256:"));

        let err = unlock(&paths.private_pem, "senha-errada").expect_err("senha errada");
        assert!(matches!(err, IdentityError::Unlock { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn public_file_is_authorized_keys_line() {
        let dir = temp_dir("pub");
        let paths = generate(&dir, "outra-senha").expect("gera");

        let line = std::fs::read_to_string(&paths.public_openssh).expect("pub");
        assert!(line.starts_with("ssh-ed25519 AAAA"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
