//! Parte de `models` (extraída para manter os arquivos revisáveis).

use super::*;

/// Lista os `.gguf` com tamanho, sem hash (instantâneo, thread de UI).
pub fn quick_inventory(dir: &Path) -> Result<Vec<ModelArtifact>, ModelsError> {
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
        let size_bytes = entry
            .metadata()
            .map_err(|err| ModelsError::Unreadable {
                dir: path.display().to_string(),
                detail: err.to_string(),
            })?
            .len();
        artifacts.push(ModelArtifact {
            file_name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_string(),
            size_bytes,
            sha256_hex: String::new(),
            manifest_status: ManifestStatus::Pendente,
        });
    }
    artifacts.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(artifacts)
}

/// Cruzamento arquivo+SHA frente ao manifesto (T-890-08, G-09..11):
/// `Pendente` sem hash; `Ok` confere; `Desviado` difere (o artefato NÃO é o
/// congelado); `SemRegistro` fora do manifesto.
#[must_use]
pub fn manifest_status(
    manifest: &ModelsManifest,
    file_name: &str,
    sha256_hex: &str,
) -> ManifestStatus {
    if sha256_hex.is_empty() {
        return ManifestStatus::Pendente;
    }
    match manifest.expected(file_name) {
        Some(expected) if expected == sha256_hex.to_ascii_lowercase() => ManifestStatus::Ok,
        Some(_) => ManifestStatus::Desviado,
        None => ManifestStatus::SemRegistro,
    }
}

/// SHA-256 de um arquivo em blocos (chamado na thread de hash).
pub fn hash_file(dir: &Path, file_name: &str) -> Result<String, ModelsError> {
    hash_file_with_progress(dir, file_name, None, |_, _, _| {})
}

/// SHA-256 com progresso intra-arquivo e cancelamento imediato (PRD 3.7:
/// taxa MB/s medida e ETA honesto). `on_progress(bytes_lidos, total,
/// bytes_por_segundo)` é chamado no máximo a cada ~150 ms.
pub fn hash_file_with_progress(
    dir: &Path,
    file_name: &str,
    stop: Option<&AtomicBool>,
    mut on_progress: impl FnMut(u64, u64, u64),
) -> Result<String, ModelsError> {
    let path = dir.join(file_name);
    let fail = |detail: String| ModelsError::Unreadable {
        dir: path.display().to_string(),
        detail,
    };
    let mut file = std::fs::File::open(&path).map_err(|err| fail(err.to_string()))?;
    let total = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    let mut buf = [0u8; 64 * 1024];
    let mut read_total: u64 = 0;
    let started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();
    loop {
        if stop.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(ModelsError::Cancelled);
        }
        let n = file.read(&mut buf).map_err(|err| fail(err.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        read_total += n as u64;
        if last_emit.elapsed() >= std::time::Duration::from_millis(150) {
            let elapsed = started.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 {
                (read_total as f64 / elapsed) as u64
            } else {
                0
            };
            on_progress(read_total, total, rate);
            last_emit = std::time::Instant::now();
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Progresso do hash em segundo plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
    /// Bytes lidos do arquivo corrente (taxa/ETA da 3.7).
    pub current_bytes: u64,
    /// Tamanho total do arquivo corrente (0 se desconhecido).
    pub current_total: u64,
    /// Taxa medida do arquivo corrente (bytes/s, medição real).
    pub rate_bps: u64,
}

impl HashProgress {
    /// Fração do arquivo corrente (0.0–1.0; 0 sem total conhecido).
    #[must_use]
    pub fn current_fraction(&self) -> f32 {
        if self.current_total == 0 {
            0.0
        } else {
            (self.current_bytes as f32 / self.current_total as f32).clamp(0.0, 1.0)
        }
    }

    /// ETA do arquivo corrente em segundos (`None` sem taxa medida).
    #[must_use]
    pub fn current_eta_secs(&self) -> Option<u64> {
        if self.rate_bps == 0 || self.current_bytes >= self.current_total {
            return None;
        }
        Some((self.current_total - self.current_bytes) / self.rate_bps)
    }
}

pub(crate) enum HashMsg {
    FileDone {
        index: usize,
        sha: String,
    },
    Progress {
        bytes_done: u64,
        file_total: u64,
        rate_bps: u64,
    },
    Finished,
}
