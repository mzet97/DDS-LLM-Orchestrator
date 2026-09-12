//! Modelos no Studio: inventário de artefatos GGUF em disco (P4 local).
//!
//! Distinção do SDD: aqui vai o **artefato de arquivos** (nome, tamanho,
//! SHA-256) — modelo carregado e cópia por nó são estados de outros painéis.
//! Nunca presume que um arquivo é um modelo completo e válido.

use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Artefato de modelo em disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifact {
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256_hex: String,
}

/// Erros do inventário (fronteira GUI ↔ sistema de arquivos).
#[derive(Debug, Error)]
pub enum ModelsError {
    /// Diretório inacessível ou ilegível.
    #[error("diretorio de modelos inacessivel {dir}: {detail}")]
    Unreadable { dir: String, detail: String },
}

/// Lista os `.gguf` do diretório com tamanho + SHA-256 (leitura em blocos).
pub fn inventory(dir: &Path) -> Result<Vec<ModelArtifact>, ModelsError> {
    let entries = std::fs::read_dir(dir).map_err(|err| ModelsError::Unreadable {
        dir: dir.display().to_string(),
        detail: err.to_string(),
    })?;
    let mut artifacts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| ModelsError::Unreadable {
            dir: dir.display().to_string(),
            detail: err.to_string(),
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("gguf") {
            continue;
        }
        artifacts.push(artifact_of(&path)?);
    }
    artifacts.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(artifacts)
}

fn artifact_of(path: &Path) -> Result<ModelArtifact, ModelsError> {
    let fail = |detail: String| ModelsError::Unreadable {
        dir: path.display().to_string(),
        detail,
    };
    let mut file = std::fs::File::open(path).map_err(|err| fail(err.to_string()))?;
    let size_bytes = file.metadata().map_err(|err| fail(err.to_string()))?.len();
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|err| fail(err.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(ModelArtifact {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string(),
        size_bytes,
        sha256_hex: format!("{:x}", hasher.finalize()),
    })
}

/// Estado do painel de modelos.
#[derive(Debug, Clone)]
pub struct ModelsState {
    pub dir: PathBuf,
    pub list: Vec<ModelArtifact>,
    pub error: String,
}

impl ModelsState {
    /// Padrão honesto: diretório de modelos do checkout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dir: PathBuf::from("/home/mzet/projetos/tese/models"),
            list: Vec::new(),
            error: String::new(),
        }
    }

    /// Reexecuta o inventário; erro preserva a lista e registra o motivo.
    pub fn refresh(&mut self) {
        match inventory(&self.dir.clone()) {
            Ok(list) => {
                self.list = list;
                self.error.clear();
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }
}

impl Default for ModelsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("studio-models-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp criado");
        std::fs::write(dir.join("a-Q4_K_M.gguf"), b"conteudo-a").expect("fixture a");
        std::fs::write(dir.join("b-Q4_K_M.gguf"), b"conteudo-b-b").expect("fixture b");
        std::fs::write(dir.join("nota.txt"), b"ignorado").expect("nao-gguf");
        dir
    }

    #[test]
    fn lists_only_gguf_with_size_and_stable_digest() {
        let dir = fixture_dir();
        let first = inventory(&dir).expect("inventario funciona");
        let second = inventory(&dir).expect("segunda leitura funciona");
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(first.len(), 2);
        assert_eq!(first[0].file_name, "a-Q4_K_M.gguf");
        assert_eq!(first[0].size_bytes, 10);
        assert_eq!(first[1].size_bytes, 12);
        assert_eq!(first, second, "digest determinístico");
        assert_eq!(first[0].sha256_hex.len(), 64);
        assert_ne!(first[0].sha256_hex, first[1].sha256_hex);
    }

    #[test]
    fn missing_dir_becomes_typed_error() {
        let dir = std::env::temp_dir().join("studio-models-inexistente-xyz");

        let err = inventory(&dir).expect_err("ausente deve falhar");

        assert!(matches!(err, ModelsError::Unreadable { .. }));
    }
}
