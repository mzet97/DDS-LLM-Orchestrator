//! Serviço Policy Engine — fonte da verdade das políticas de segurança.
//!
//! Porte de `src/orchestrator/policy_engine/main.py` (Python):
//!
//! - lê `policies.json` no startup e re-publica a cada 60 s (late-join);
//! - detecta mudança por versão **e** conteúdo antes de publicar
//!   (`new_version == self._current_version and self._current_policy == policy_data`);
//! - publica `Security.PolicySnapshot` (TRANSIENT_LOCAL) com
//!   `published_by = "policy-engine-v1"`;
//! - cacheia o documento localmente com TTL de 300 s;
//! - falha de leitura/parse do arquivo só loga e mantém a versão atual
//!   (o serviço não cai — comportamento do Python).
//!
//! Extensão Rust: assina `Security.PolicyUpdate` e aplica deltas
//! (`ADD_RULE`/`UPDATE_RULE`/`REMOVE_RULE`), republicando o snapshot
//! resultante — o ciclo pub/sub completo que o Python só declarava no modelo.
//!
//! O serviço é genérico sobre [`DataSpaceApi`]: roda contra o `DataSpace`
//! real (feature `dds`) ou contra o `InMemoryDataSpace` em testes.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, SecurityPolicyUpdate};
use dds_dataspace::api::DataSpaceApi;
use futures::StreamExt;
use serde_json::Value;

use crate::cache::{PolicyCache, DEFAULT_CACHE_TTL};
use crate::error::PolicyError;
use crate::now_ns;
use crate::rules::PolicyDocument;

/// Intervalo de re-publicação (60 s, fiel ao Python).
pub const DEFAULT_REPUBLISH_INTERVAL: Duration = Duration::from_secs(60);
/// `policy_id` padrão (`"default"` no Python).
pub const DEFAULT_POLICY_ID: &str = "default";
/// `published_by` do snapshot — mesmo valor do Python.
pub const DEFAULT_PUBLISHED_BY: &str = "policy-engine-v1";

/// Estado corrente de uma política (versão + documento).
#[derive(Debug, Clone)]
struct PolicyState {
    version: i32,
    document: PolicyDocument,
}

/// Serviço Policy Engine (ver docs do módulo).
pub struct PolicyEngineService<D: DataSpaceApi> {
    data_space: Arc<D>,
    policy_file: PathBuf,
    policy_id: String,
    published_by: String,
    republish_interval: Duration,
    cache: PolicyCache,
    states: Arc<DashMap<String, PolicyState>>,
}

impl<D: DataSpaceApi> PolicyEngineService<D> {
    pub fn new(data_space: Arc<D>, policy_file: impl Into<PathBuf>) -> Self {
        Self {
            data_space,
            policy_file: policy_file.into(),
            policy_id: DEFAULT_POLICY_ID.to_string(),
            published_by: DEFAULT_PUBLISHED_BY.to_string(),
            republish_interval: DEFAULT_REPUBLISH_INTERVAL,
            cache: PolicyCache::new(),
            states: Arc::new(DashMap::new()),
        }
    }

    pub fn with_policy_id(mut self, policy_id: impl Into<String>) -> Self {
        self.policy_id = policy_id.into();
        self
    }

    pub fn with_published_by(mut self, published_by: impl Into<String>) -> Self {
        self.published_by = published_by.into();
        self
    }

    pub fn with_republish_interval(mut self, interval: Duration) -> Self {
        self.republish_interval = interval;
        self
    }

    /// Cache local de políticas (TTL 300 s).
    pub fn cache(&self) -> &PolicyCache {
        &self.cache
    }

    /// Estado corrente de uma política: `(versão, documento)`.
    pub fn current_state(&self, policy_id: &str) -> Option<(i32, PolicyDocument)> {
        self.states
            .get(policy_id)
            .map(|s| (s.version, s.document.clone()))
    }

    /// Lê `policies.json` e publica o snapshot se a versão/conteúdo mudou
    /// (porte de `_load_and_publish`). Retorna `true` quando publicou.
    ///
    /// Falha de leitura/parse só loga e retorna `Ok(false)` — como o Python,
    /// o serviço mantém a versão anterior em vez de cair.
    pub async fn load_and_publish(&self) -> Result<bool, PolicyError> {
        let content = match std::fs::read_to_string(&self.policy_file) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    path = ?self.policy_file,
                    error = %e,
                    "falha ao carregar policies; mantendo versão atual"
                );
                return Ok(false);
            }
        };
        let doc = match PolicyDocument::from_json_str(&content) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(
                    path = ?self.policy_file,
                    error = %e,
                    "falha ao parsear policies; mantendo versão atual"
                );
                return Ok(false);
            }
        };

        // Detecta mudança: mesma versão E mesmo conteúdo → nada a publicar.
        let new_version = doc.version();
        if let Some(cur) = self.states.get(&self.policy_id) {
            if cur.version == new_version && cur.document.as_value() == doc.as_value() {
                return Ok(false);
            }
        }

        self.states.insert(
            self.policy_id.clone(),
            PolicyState {
                version: new_version,
                document: doc.clone(),
            },
        );
        self.cache
            .set(&self.policy_id, doc.as_value().clone(), DEFAULT_CACHE_TTL);
        tracing::info!(version = new_version, "policy cacheada localmente");

        self.publish_snapshot(&self.policy_id, new_version, doc.as_value())
            .await?;
        tracing::info!(version = new_version, "policy publicada");
        Ok(true)
    }

    /// Aplica um `SecurityPolicyUpdate` e republica o snapshot resultante.
    ///
    /// Se não há estado local para o `policy_id`, o delta é aplicado sobre um
    /// documento vazio. Divergência de `previous_version` só gera warning
    /// (last-writer-wins — o snapshot resultante carrega a verdade).
    pub async fn handle_update(&self, update: &SecurityPolicyUpdate) -> Result<bool, PolicyError> {
        let delta: Value = serde_json::from_str(&update.rule_delta_json)?;
        let (version, doc_value);
        {
            let mut entry = self
                .states
                .entry(update.policy_id.clone())
                .or_insert_with(|| PolicyState {
                    version: 0,
                    document: PolicyDocument::empty(),
                });
            if update.previous_version != entry.version {
                tracing::warn!(
                    policy_id = %update.policy_id,
                    expected = update.previous_version,
                    current = entry.version,
                    "previous_version diverge do estado local; aplicando mesmo assim"
                );
            }
            entry.document.apply_delta(&update.operation, &delta)?;
            entry.version = update.new_version;
            entry.document.set_version(update.new_version);
            version = entry.version;
            doc_value = entry.document.as_value().clone();
        }

        self.cache
            .set(&update.policy_id, doc_value.clone(), DEFAULT_CACHE_TTL);
        self.publish_snapshot(&update.policy_id, version, &doc_value)
            .await?;
        tracing::info!(
            policy_id = %update.policy_id,
            new_version = version,
            operation = %update.operation,
            "delta aplicado e policy republicada"
        );
        Ok(true)
    }

    /// Loop do serviço (porte de `run`): carga inicial + re-publicação
    /// periódica + aplicação contínua de deltas. Roda até o stream de
    /// updates fechar; o shutdown (SIGINT) fica a cargo do chamador (`select!`).
    pub async fn run(&self) -> Result<(), PolicyError> {
        tracing::info!(policy_file = ?self.policy_file, "Policy Engine iniciado");
        self.load_and_publish().await?;

        let mut updates = self.data_space.subscribe_security_updates();
        // 1ª tick só após o intervalo (a carga inicial já aconteceu acima).
        let start = tokio::time::Instant::now() + self.republish_interval;
        let mut ticker = tokio::time::interval_at(start, self.republish_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                maybe_update = updates.next() => {
                    match maybe_update {
                        Some(update) => {
                            if let Err(e) = self.handle_update(&update).await {
                                tracing::warn!(
                                    error = %e,
                                    "falha ao aplicar SecurityPolicyUpdate; ignorando"
                                );
                            }
                        }
                        None => {
                            tracing::warn!("stream de SecurityPolicyUpdate encerrado");
                            return Ok(());
                        }
                    }
                }
                _ = ticker.tick() => {
                    self.load_and_publish().await?;
                }
            }
        }
    }

    /// Publica o snapshot da política (mesmos campos do Python).
    async fn publish_snapshot(
        &self,
        policy_id: &str,
        version: i32,
        document: &Value,
    ) -> Result<(), PolicyError> {
        let snapshot = SecurityPolicySnapshot {
            policy_id: policy_id.to_string(),
            version,
            policy_json: document.to_string(),
            published_by: self.published_by.clone(),
            timestamp_ns: now_ns(),
        };
        self.data_space.write_security_snapshot(snapshot).await?;
        Ok(())
    }
}
