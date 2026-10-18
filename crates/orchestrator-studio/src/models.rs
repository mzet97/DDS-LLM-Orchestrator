//! Modelos no Studio: inventário de artefatos GGUF em disco (P4 local).
//!
//! Distinção do SDD: aqui vai o **artefato de arquivos** (nome, tamanho,
//! SHA-256) — modelo carregado e cópia por nó são estados de outros painéis.
//! Nunca presume que um arquivo é um modelo completo e válido.
//!
//! Duas fases para nunca travar a UI: listagem instantânea (só metadados) e
//! hash em thread dedicada com progresso e cancelamento.
//!
//! Diretório padrão (T-830-06), nesta ordem: `STUDIO_MODELS_DIR` (se definido)
//! → `$HOME/tese/models` (se existir) → vazio (a UI sugere configurar
//! `STUDIO_MODELS_DIR`; o campo de caminho continua editável).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use thiserror::Error;

/// Artefato de modelo em disco (`sha256_hex` vazio = hash pendente).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifact {
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256_hex: String,
    /// Cruzamento com o manifesto congelado (T-890-08, G-09..11).
    pub manifest_status: ManifestStatus,
}

/// Erros do inventário (fronteira GUI ↔ sistema de arquivos).
#[derive(Debug, Error)]
pub enum ModelsError {
    /// Diretório inacessível ou ilegível.
    #[error("diretorio de modelos inacessivel {dir}: {detail}")]
    Unreadable { dir: String, detail: String },
    /// Falha de leitura/parse do manifesto GGUF×SHA (T-890-08, G-09..11).
    #[error("manifesto invalido: {0}")]
    Manifest(String),
    /// Hash interrompido pelo usuário (botão Cancelar da 3.7).
    #[error("hash cancelado")]
    Cancelled,
}

/// Status do artefato frente ao manifesto (T-890-08, G-09..11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManifestStatus {
    /// Sem hash calculado ainda.
    #[default]
    Pendente,
    /// SHA confere com o manifesto.
    Ok,
    /// SHA DIFERE do manifesto — artefato não é o congelado.
    Desviado,
    /// Arquivo não está no manifesto (fora do escopo congelado).
    SemRegistro,
}

impl ManifestStatus {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pendente => "pendente",
            Self::Ok => "OK",
            Self::Desviado => "DESVIADO",
            Self::SemRegistro => "sem registro",
        }
    }
}

/// Manifesto congelado: nome do arquivo → SHA-256 (hex minúsculo, 64 chars).
/// Formato canônico: `{"models": {"<arquivo>.gguf": "<sha256>"}}`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelsManifest {
    entries: std::collections::BTreeMap<String, String>,
}

impl ModelsManifest {
    /// Parse + validação (64 hex por entrada; aceita maiúsculas, normaliza).
    pub fn from_json(json: &str) -> Result<Self, ModelsError> {
        let parsed: serde_json::Value = serde_json::from_str(json)
            .map_err(|err| ModelsError::Manifest(format!("json: {err}")))?;
        let models = parsed
            .get("models")
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                ModelsError::Manifest(String::from(
                    "esperado objeto \"models\" mapeando arquivo → sha256",
                ))
            })?;
        let mut entries = std::collections::BTreeMap::new();
        for (name, sha) in models {
            let sha = sha
                .as_str()
                .ok_or_else(|| ModelsError::Manifest(format!("{name}: sha não é string")))?;
            let lowered = sha.trim().to_ascii_lowercase();
            if lowered.len() != 64 || !lowered.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(ModelsError::Manifest(format!(
                    "{name}: sha256 deve ter 64 hex, veio \"{sha}\""
                )));
            }
            entries.insert(name.clone(), lowered);
        }
        Ok(Self { entries })
    }

    /// SHA esperado de um arquivo (`None` = sem registro).
    #[must_use]
    pub fn expected(&self, file_name: &str) -> Option<&str> {
        self.entries.get(file_name).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

mod models_poll;
mod models_scan;
mod models_state;

pub(crate) use models_scan::HashMsg;
pub use models_scan::{
    hash_file, hash_file_with_progress, manifest_status, quick_inventory, HashProgress,
};
#[cfg(test)]
pub(crate) use models_state::resolve_models_dir;
pub use models_state::{default_manifest_path, default_models_dir, ModelsState};

#[cfg(test)]
mod tests_manifest;
#[cfg(test)]
mod tests_scan;
