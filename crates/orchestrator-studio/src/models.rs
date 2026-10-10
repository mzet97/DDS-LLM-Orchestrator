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

enum HashMsg {
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

/// Estado do painel de modelos com hash assíncrono e cancelamento.
pub struct ModelsState {
    pub dir: PathBuf,
    pub list: Vec<ModelArtifact>,
    pub hashing: Option<HashProgress>,
    pub error: String,
    /// T-890-08 (G-09..11): manifesto congelado carregado (cruzamento
    /// GGUF×SHA); `None` = nenhum manifesto carregado.
    pub manifest: Option<ModelsManifest>,
    pub manifest_path: String,
    pub manifest_error: String,
    /// Filtro da tabela (tela 3.7): 0=todos, 1=divergentes, 2=pendentes.
    pub filter: u8,
    /// Aviso curto da última ação (ex.: caminho do relatório exportado).
    pub notice: String,
    receiver: Option<mpsc::Receiver<HashMsg>>,
    cancel: Option<Arc<AtomicBool>>,
}

/// Ordem de resolução do diretório de modelos (T-830-06): `env_dir` vence;
/// sem env, `$HOME/tese/models` somente se existir; senão vazio (a UI pede
/// `STUDIO_MODELS_DIR`). Função pura (env/home como parâmetros) para teste
/// direto, sem ambiente global.
fn resolve_models_dir(env_dir: Option<&str>, home: Option<&str>) -> String {
    if let Some(dir) = env_dir.map(str::trim).filter(|dir| !dir.is_empty()) {
        return String::from(dir);
    }
    match home.map(Path::new) {
        Some(home) if home.join("tese/models").is_dir() => {
            home.join("tese/models").display().to_string()
        }
        _ => String::new(),
    }
}

/// Caminho padrão do manifesto: o lock da campanha (`benchmarks/orchestration
/// /locks/models-manifest.json`) quando existir; senão vazio (a UI pede).
#[must_use]
pub fn default_manifest_path() -> String {
    let candidates = [
        "benchmarks/orchestration/locks/models-manifest.json",
        "../benchmarks/orchestration/locks/models-manifest.json",
    ];
    candidates
        .iter()
        .find(|c| Path::new(c).is_file())
        .map(|c| c.to_string())
        .unwrap_or_default()
}

/// Diretório padrão do primeiro render (T-830-06); ver [`resolve_models_dir`].
#[must_use]
pub fn default_models_dir() -> String {
    let env_dir = std::env::var("STUDIO_MODELS_DIR").ok();
    let home = std::env::var("HOME").ok();
    resolve_models_dir(env_dir.as_deref(), home.as_deref())
}

impl ModelsState {
    /// Padrão honesto: resolução por ambiente (T-830-06), sem caminho de
    /// usuário cravado no código.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dir: PathBuf::from(default_models_dir()),
            list: Vec::new(),
            hashing: None,
            error: String::new(),
            manifest: None,
            manifest_path: default_manifest_path(),
            manifest_error: String::new(),
            filter: 0,
            notice: String::new(),
            receiver: None,
            cancel: None,
        }
    }

    /// Exporta o relatório do inventário (JSON) no diretório de modelos —
    /// arquivo, tamanho, sha calculado e status frente ao manifesto.
    pub fn export_report(&mut self) {
        if self.list.is_empty() {
            self.notice = String::from("nada a exportar — inventarie primeiro");
            return;
        }
        let report: Vec<serde_json::Value> = self
            .list
            .iter()
            .map(|artifact| {
                serde_json::json!({
                    "file": artifact.file_name,
                    "size_bytes": artifact.size_bytes,
                    "sha256": artifact.sha256_hex,
                    "manifest_status": artifact.manifest_status.label(),
                })
            })
            .collect();
        let path = if self.dir.as_os_str().is_empty() {
            std::path::PathBuf::from("studio-models-report.json")
        } else {
            self.dir.join("studio-models-report.json")
        };
        match serde_json::to_string_pretty(&report) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => {
                    self.notice = format!("relatório exportado: {}", path.display());
                }
                Err(err) => {
                    self.notice = format!("falha ao exportar: {err}");
                }
            },
            Err(err) => {
                self.notice = format!("falha ao serializar: {err}");
            }
        }
    }

    /// `true` enquanto a thread de hash trabalha.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.hashing.is_some()
    }

    /// Lista instantânea + dispara o hash em segundo plano.
    pub fn refresh(&mut self) {
        self.cancel();
        match quick_inventory(&self.dir.clone()) {
            Ok(list) => {
                self.error.clear();
                self.list = list;
                self.spawn_hash();
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }

    /// Re-verifica a lista ATUAL sem re-inventariar (botão "Verificar Tudo"
    /// da 3.7): limpa SHAs/status e re-hasha tudo do zero. Lista vazia = no-op.
    pub fn verify_all(&mut self) {
        if self.list.is_empty() || self.is_busy() {
            return;
        }
        for artifact in &mut self.list {
            artifact.sha256_hex.clear();
            artifact.manifest_status = ManifestStatus::Pendente;
        }
        self.error.clear();
        self.spawn_hash();
    }

    /// Carrega o manifesto do caminho dado e RECUCOMPÕE os status da lista
    /// (T-890-08, G-09..11). Erro não descarta o manifesto anterior.
    pub fn load_manifest(&mut self) {
        let path = self.manifest_path.trim();
        if path.is_empty() {
            self.manifest_error = String::from("caminho do manifesto vazio");
            return;
        }
        match std::fs::read_to_string(path) {
            Ok(json) => match ModelsManifest::from_json(&json) {
                Ok(manifest) => {
                    self.manifest_error.clear();
                    for artifact in &mut self.list {
                        artifact.manifest_status =
                            manifest_status(&manifest, &artifact.file_name, &artifact.sha256_hex);
                    }
                    self.manifest = Some(manifest);
                }
                Err(err) => self.manifest_error = err.to_string(),
            },
            Err(err) => {
                self.manifest_error = format!("{}: {err}", path);
            }
        }
    }

    /// Status do arquivo na posição `index` frente ao manifesto carregado.
    fn manifest_status_for_index(&self, index: usize, sha: &str) -> ManifestStatus {
        match (&self.manifest, self.list.get(index)) {
            (Some(manifest), Some(artifact)) => manifest_status(manifest, &artifact.file_name, sha),
            _ => ManifestStatus::Pendente,
        }
    }

    /// Cancela o hash em curso; lista parcial é mantida com SHAs pendentes.
    pub fn cancel(&mut self) {
        if let Some(flag) = self.cancel.take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.receiver = None;
        if self.hashing.take().is_some() {
            self.error = String::from("hash cancelado; SHAs parciais mantidos");
        }
    }

    /// Drena o progresso; chamar a cada frame enquanto `is_busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    HashMsg::FileDone { index, sha } => {
                        let status = self.manifest_status_for_index(index, &sha);
                        if let Some(artifact) = self.list.get_mut(index) {
                            artifact.sha256_hex = sha;
                            artifact.manifest_status = status;
                        }
                        if let Some(progress) = self.hashing.as_mut() {
                            progress.done = progress.done.saturating_add(1);
                            progress.current = self
                                .list
                                .get(progress.done)
                                .map(|artifact| artifact.file_name.clone())
                                .unwrap_or_default();
                            progress.current_bytes = 0;
                            progress.current_total = 0;
                            progress.rate_bps = 0;
                        }
                    }
                    HashMsg::Progress {
                        bytes_done,
                        file_total,
                        rate_bps,
                    } => {
                        if let Some(progress) = self.hashing.as_mut() {
                            progress.current_bytes = bytes_done;
                            progress.current_total = file_total;
                            progress.rate_bps = rate_bps;
                        }
                    }
                    HashMsg::Finished => {
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
            self.cancel = None;
            self.hashing = None;
        }
    }

    fn spawn_hash(&mut self) {
        if self.list.is_empty() {
            return;
        }
        let dir = self.dir.clone();
        let names: Vec<String> = self
            .list
            .iter()
            .map(|item| item.file_name.clone())
            .collect();
        let (tx, rx) = mpsc::channel();
        let flag = Arc::new(AtomicBool::new(false));
        let stop = flag.clone();
        std::thread::spawn(move || {
            for (index, name) in names.iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let tx_progress = tx.clone();
                let result =
                    hash_file_with_progress(&dir, name, Some(&stop), |bytes, total, rate| {
                        // Throttle extra na thread de UI: o worker já limita a
                        // ~6.7 msg/s por arquivo; canal absorve picos.
                        let _ = tx_progress.send(HashMsg::Progress {
                            bytes_done: bytes,
                            file_total: total,
                            rate_bps: rate,
                        });
                    });
                match result {
                    Ok(sha) => {
                        let _ = tx.send(HashMsg::FileDone { index, sha });
                    }
                    Err(ModelsError::Cancelled) => break,
                    Err(_) => continue,
                }
            }
            let _ = tx.send(HashMsg::Finished);
        });
        self.receiver = Some(rx);
        self.cancel = Some(flag);
        self.hashing = Some(HashProgress {
            done: 0,
            total: self.list.len(),
            current: self
                .list
                .first()
                .map(|item| item.file_name.clone())
                .unwrap_or_default(),
            current_bytes: 0,
            current_total: 0,
            rate_bps: 0,
        });
    }
}

impl Default for ModelsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{manifest_status, ManifestStatus, ModelsError, ModelsManifest, ModelsState};

    const SHA_A: &str = "bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517";

    // T-890-08 (G-09..11): parse do manifesto com validação de 64 hex.
    #[test]
    fn manifest_parse_validates_sha_and_normalizes_case() {
        let manifest =
            ModelsManifest::from_json(&format!("{{\"models\": {{\"qwen.gguf\": \"{SHA_A}\"}}}}"))
                .expect("manifesto válido");
        assert_eq!(manifest.expected("qwen.gguf"), Some(SHA_A));
        assert_eq!(manifest.len(), 1);

        // MAIÚSCULAS normalizadas para minúsculas.
        let upper = ModelsManifest::from_json(&format!(
            "{{\"models\": {{\"qwen.gguf\": \"{}\"}}}}",
            SHA_A.to_ascii_uppercase()
        ))
        .expect("sha em maiúsculas é aceito");
        assert_eq!(upper.expected("qwen.gguf"), Some(SHA_A));
    }

    #[test]
    fn manifest_parse_rejects_malformed_inputs() {
        assert!(matches!(
            ModelsManifest::from_json("não é json"),
            Err(ModelsError::Manifest(_))
        ));
        // sem a chave "models"
        assert!(matches!(
            ModelsManifest::from_json("{\"outro\": 1}"),
            Err(ModelsError::Manifest(_))
        ));
        // sha curto
        assert!(matches!(
            ModelsManifest::from_json("{\"models\": {\"a.gguf\": \"abcd\"}}"),
            Err(ModelsError::Manifest(_))
        ));
    }

    // Cruzamento: pendente sem hash, OK confere, DESVIADO difere,
    // SemRegistro fora do manifesto (o coração do G-09..11).
    #[test]
    fn manifest_status_classifies_ok_deviant_unregistered() {
        let manifest =
            ModelsManifest::from_json(&format!("{{\"models\": {{\"qwen.gguf\": \"{SHA_A}\"}}}}"))
                .expect("manifesto");
        assert_eq!(
            manifest_status(&manifest, "qwen.gguf", ""),
            ManifestStatus::Pendente
        );
        assert_eq!(
            manifest_status(&manifest, "qwen.gguf", SHA_A),
            ManifestStatus::Ok
        );
        assert_eq!(
            manifest_status(&manifest, "qwen.gguf", &"0".repeat(64)),
            ManifestStatus::Desviado
        );
        assert_eq!(
            manifest_status(&manifest, "desconhecido.gguf", SHA_A),
            ManifestStatus::SemRegistro
        );
    }

    /// Integração: FileDone via poll aplica o status do manifesto carregado.
    #[test]
    fn poll_applies_manifest_status_after_hash() {
        let dir = std::env::temp_dir().join(format!(
            "studio-manifest-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("qwen.gguf"), b"conteudo do gguf de teste").expect("escreve");
        let expected_sha = {
            use sha2::Digest;
            let mut h = sha2::Sha256::new();
            h.update(b"conteudo do gguf de teste");
            format!("{:x}", h.finalize())
        };
        let mut state = ModelsState::new();
        state.dir = dir.clone();
        state.refresh();
        state.manifest = Some(
            ModelsManifest::from_json(&format!(
                "{{\"models\": {{\"qwen.gguf\": \"{expected_sha}\"}}}}"
            ))
            .expect("manifesto"),
        );
        for _ in 0..400 {
            state.poll();
            if !state.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!state.is_busy(), "hash não concluiu");
        let artifact = &state.list[0];
        assert_eq!(artifact.sha256_hex, expected_sha);
        assert_eq!(artifact.manifest_status, ManifestStatus::Ok);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// DESVIADO: sha real difere do congelado (cópia corrompida/trocada).
    #[test]
    fn poll_marks_deviant_when_sha_differs_from_manifest() {
        let dir = std::env::temp_dir().join(format!(
            "studio-deviant-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("qwen.gguf"), b"arquivo adulterado").expect("escreve");
        let mut state = ModelsState::new();
        state.dir = dir.clone();
        state.refresh();
        state.manifest = Some(
            ModelsManifest::from_json(&format!("{{\"models\": {{\"qwen.gguf\": \"{SHA_A}\"}}}}"))
                .expect("manifesto"),
        );
        for _ in 0..400 {
            state.poll();
            if !state.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(state.list[0].manifest_status, ManifestStatus::Desviado);
        std::fs::remove_dir_all(&dir).ok();
    }

    use super::*;

    fn fixture_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("studio-models-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp criado");
        std::fs::write(dir.join("a-Q4_K_M.gguf"), b"conteudo-a").expect("fixture a");
        std::fs::write(dir.join("b-Q4_K_M.gguf"), b"conteudo-b-b").expect("fixture b");
        std::fs::write(dir.join("nota.txt"), b"ignorado").expect("nao-gguf");
        dir
    }

    #[test]
    fn quick_lists_without_hashing() {
        let dir = fixture_dir("quick");
        let list = quick_inventory(&dir).expect("lista");
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(list.len(), 2);
        assert_eq!(list[0].size_bytes, 10);
        assert!(list.iter().all(|item| item.sha256_hex.is_empty()));
    }

    #[test]
    fn hash_is_stable_and_hex() {
        let dir = fixture_dir("hash");
        let first = hash_file(&dir, "a-Q4_K_M.gguf").expect("hash");
        let second = hash_file(&dir, "a-Q4_K_M.gguf").expect("hash de novo");
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    /// PRD 3.7: fração e ETA do arquivo corrente derivam da taxa medida.
    #[test]
    fn hash_progress_derives_fraction_and_eta() {
        let progress = HashProgress {
            done: 1,
            total: 3,
            current: String::from("b.gguf"),
            current_bytes: 250,
            current_total: 1_000,
            rate_bps: 50,
        };
        assert!((progress.current_fraction() - 0.25).abs() < f32::EPSILON);
        assert_eq!(
            progress.current_eta_secs(),
            Some(15),
            "750 bytes restantes a 50 B/s"
        );

        let unknown = HashProgress {
            done: 0,
            total: 1,
            current: String::new(),
            current_bytes: 0,
            current_total: 0,
            rate_bps: 0,
        };
        assert_eq!(unknown.current_fraction(), 0.0);
        assert_eq!(unknown.current_eta_secs(), None);
    }

    /// Cancelamento intra-arquivo: flag interrompe o hash imediatamente.
    #[test]
    fn hash_file_with_progress_stops_on_flag() {
        let dir = fixture_dir("cancel");
        let stop = AtomicBool::new(true); // já cancelado
        let err = hash_file_with_progress(&dir, "a-Q4_K_M.gguf", Some(&stop), |_, _, _| {})
            .expect_err("flag cancelada aborta");
        assert!(matches!(err, ModelsError::Cancelled));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn background_hash_completes_through_poll() {
        let dir = fixture_dir("poll");
        let mut state = ModelsState::new();
        state.dir = dir.clone();
        state.refresh();

        assert!(state.is_busy());
        for _ in 0..1000 {
            state.poll();
            if !state.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let _ = std::fs::remove_dir_all(&dir);

        assert!(!state.is_busy());
        assert!(state.list.iter().all(|item| item.sha256_hex.len() == 64));
    }

    #[test]
    fn missing_dir_becomes_typed_error() {
        let dir = std::env::temp_dir().join("studio-models-inexistente-xyz");

        let err = quick_inventory(&dir).expect_err("ausente deve falhar");

        assert!(matches!(err, ModelsError::Unreadable { .. }));
    }

    #[test]
    fn env_dir_wins_over_home_fallback() {
        // T-830-06: env definido (mesmo apontando fora de HOME) vence.
        assert_eq!(
            resolve_models_dir(Some("/opt/modelos"), Some("/home/alguem")),
            "/opt/modelos"
        );
        // Espaços nas bordas não criam diretório fantasma.
        assert_eq!(resolve_models_dir(Some("  "), Some("/home/alguem")), "");
    }

    #[test]
    fn home_fallback_only_when_tese_models_exists() {
        let home = std::env::temp_dir().join(format!("studio-models-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("tese/models")).expect("fixture home");
        let expected = home.join("tese/models").display().to_string();

        assert_eq!(
            resolve_models_dir(None, home.to_str()),
            expected,
            "$HOME/tese/models existente é o fallback"
        );
        let _ = std::fs::remove_dir_all(&home);

        // Sem o diretório (ou sem HOME): vazio — a UI sugere STUDIO_MODELS_DIR.
        assert_eq!(resolve_models_dir(None, home.to_str()), "");
        assert_eq!(resolve_models_dir(None, Some("/inexistente")), "");
        assert_eq!(resolve_models_dir(None, None), "");
    }
}
