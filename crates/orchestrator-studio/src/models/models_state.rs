//! Parte de `models` (extraída para manter os arquivos revisáveis).

use super::*;

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
    pub(crate) receiver: Option<mpsc::Receiver<HashMsg>>,
    pub(crate) cancel: Option<Arc<AtomicBool>>,
}

/// Ordem de resolução do diretório de modelos (T-830-06): `env_dir` vence;
/// sem env, `$HOME/tese/models` somente se existir; senão vazio (a UI pede
/// `STUDIO_MODELS_DIR`). Função pura (env/home como parâmetros) para teste
/// direto, sem ambiente global.
pub(crate) fn resolve_models_dir(env_dir: Option<&str>, home: Option<&str>) -> String {
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
    pub(super) fn manifest_status_for_index(&self, index: usize, sha: &str) -> ManifestStatus {
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
}
