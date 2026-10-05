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
}

/// Erros do inventário (fronteira GUI ↔ sistema de arquivos).
#[derive(Debug, Error)]
pub enum ModelsError {
    /// Diretório inacessível ou ilegível.
    #[error("diretorio de modelos inacessivel {dir}: {detail}")]
    Unreadable { dir: String, detail: String },
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
        });
    }
    artifacts.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(artifacts)
}

/// SHA-256 de um arquivo em blocos (chamado na thread de hash).
pub fn hash_file(dir: &Path, file_name: &str) -> Result<String, ModelsError> {
    let path = dir.join(file_name);
    let fail = |detail: String| ModelsError::Unreadable {
        dir: path.display().to_string(),
        detail,
    };
    let mut file = std::fs::File::open(&path).map_err(|err| fail(err.to_string()))?;
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
    Ok(format!("{:x}", hasher.finalize()))
}

/// Progresso do hash em segundo plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

enum HashMsg {
    FileDone { index: usize, sha: String },
    Finished,
}

/// Estado do painel de modelos com hash assíncrono e cancelamento.
pub struct ModelsState {
    pub dir: PathBuf,
    pub list: Vec<ModelArtifact>,
    pub hashing: Option<HashProgress>,
    pub error: String,
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
            receiver: None,
            cancel: None,
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
                        if let Some(artifact) = self.list.get_mut(index) {
                            artifact.sha256_hex = sha;
                        }
                        if let Some(progress) = self.hashing.as_mut() {
                            progress.done = progress.done.saturating_add(1);
                            progress.current = self
                                .list
                                .get(progress.done)
                                .map(|artifact| artifact.file_name.clone())
                                .unwrap_or_default();
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
                if let Ok(sha) = hash_file(&dir, name) {
                    let _ = tx.send(HashMsg::FileDone { index, sha });
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
