//! `LocalContextStore`: implementação em memória (dashmap) com journal
//! JSONL append-only para durabilidade leve — substitui o PostgreSQL nesta
//! etapa da migração (ver TODO em `lib.rs`).
//!
//! ## Durabilidade
//! Cada `put_snapshot`/`apply_update` grava **antes** de aplicar em memória
//! (write-ahead): uma linha JSON por operação, com `flush` a cada escrita
//! (sem fsync — "durabilidade leve"). Em `open`, o arquivo é re-executado
//! (replay) com a mesma semântica do store, restaurando o estado.
//!
//! O registro carrega `expires_at_ns` **absoluto** (calculado na escrita
//! original), então o replay preserva a expiração mesmo após restart —
//! equivalente à coluna `expires_at` do PostgreSQL.
//!
//! ## Concorrência
//! Leituras (`get`/`list_sessions`) são lock-free (dashmap). Escritas
//! (`put_snapshot`/`apply_update`) passam por um `write_lock` que serializa
//! journal + aplicação: assim a ordem do journal é exatamente a ordem de
//! aplicação por contexto (o consumidor DDS é single-task, como no Python,
//! então o lock é praticamente livre de contenção).

use crate::store::{
    apply_messages_delta, expires_from_now, now_ns, snapshot_from_update, ContextStore, StoreError,
    DEFAULT_TTL_SECONDS,
};
use dashmap::DashMap;
use dds_contract::generated::dds_llm_orchestrator::{ContextSnapshot, ContextUpdate};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;

/// Entrada interna: snapshot + expiração absoluta (ns desde epoch).
#[derive(Debug, Clone)]
struct ContextEntry {
    snapshot: ContextSnapshot,
    expires_at_ns: u64,
}

/// Registro do journal JSONL (uma linha por operação).
///
/// Os tipos gerados do IDL **não** derivam serde, então o registro espelha
/// os campos do IDL 1:1 com conversões dedicadas.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JournalRecord {
    /// Upsert de snapshot (`put_snapshot`).
    Put {
        context_id: String,
        client_id: String,
        session_id: String,
        messages_json: String,
        metadata_json: String,
        security_level: i32,
        created_at_ns: u64,
        updated_at_ns: u64,
        ttl_seconds: u32,
        expires_at_ns: u64,
    },
    /// Delta aplicado (`apply_update`).
    Update {
        context_id: String,
        update_type: i32,
        messages_delta_json: String,
        metadata_delta_json: String,
        updated_at_ns: u64,
        expires_at_ns: u64,
    },
}

impl JournalRecord {
    fn put(snapshot: &ContextSnapshot, expires_at_ns: u64) -> Self {
        Self::Put {
            context_id: snapshot.context_id.clone(),
            client_id: snapshot.client_id.clone(),
            session_id: snapshot.session_id.clone(),
            messages_json: snapshot.messages_json.clone(),
            metadata_json: snapshot.metadata_json.clone(),
            security_level: snapshot.security_level,
            created_at_ns: snapshot.created_at_ns,
            updated_at_ns: snapshot.updated_at_ns,
            ttl_seconds: snapshot.ttl_seconds,
            expires_at_ns,
        }
    }

    fn update(update: &ContextUpdate, expires_at_ns: u64) -> Self {
        Self::Update {
            context_id: update.context_id.clone(),
            update_type: update.update_type,
            messages_delta_json: update.messages_delta_json.clone(),
            metadata_delta_json: update.metadata_delta_json.clone(),
            updated_at_ns: update.updated_at_ns,
            expires_at_ns,
        }
    }
}

/// Store local: memória (dashmap, lock-free por shard) + journal JSONL.
///
/// `ahash` em vez do hasher default (Fase 2 do `OPTIMIZATION_PLAN.md`).
pub struct LocalContextStore {
    entries: DashMap<String, ContextEntry, ahash::RandomState>,
    /// Serializa escritas (journal + aplicação) para manter a ordem do WAL.
    write_lock: tokio::sync::Mutex<()>,
    /// Arquivo do journal (append). `None` em `in_memory` (testes/efêmero).
    journal: Option<tokio::sync::Mutex<tokio::fs::File>>,
    /// Caminho do journal (para logs/diagnóstico).
    journal_path: Option<PathBuf>,
    /// Contador de operações persistidas (diagnóstico).
    journaled_ops: AtomicU64,
}

impl LocalContextStore {
    /// Store volátil, sem journal (testes unitários e uso efêmero).
    pub fn in_memory() -> Self {
        Self {
            entries: DashMap::with_hasher(ahash::RandomState::default()),
            write_lock: tokio::sync::Mutex::new(()),
            journal: None,
            journal_path: None,
            journaled_ops: AtomicU64::new(0),
        }
    }

    /// Abre (ou cria) o journal em `path`, re-executa os registros
    /// existentes e retorna o store pronto para uso.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let mut store = Self::in_memory();
        store.journal_path = Some(path.clone());

        // Replay: aplica os registros na ordem, com a mesma semântica do store.
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => store.replay(&content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(StoreError::Io(e)),
        }

        // Abre o handle de append para as próximas escritas.
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(StoreError::Io)?;
        store.journal = Some(tokio::sync::Mutex::new(file));
        Ok(store)
    }

    /// Re-executa o conteúdo do journal. Linhas corrompidas são logadas e
    /// puladas (o journal é durabilidade leve, não fonte canônica de verdade).
    fn replay(&self, content: &str) {
        let mut applied = 0u64;
        for (lineno, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let record: JournalRecord = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        line = lineno + 1,
                        error = %e,
                        "journal: linha corrompida ignorada"
                    );
                    continue;
                }
            };
            match record {
                JournalRecord::Put {
                    context_id,
                    client_id,
                    session_id,
                    messages_json,
                    metadata_json,
                    security_level,
                    created_at_ns,
                    updated_at_ns,
                    ttl_seconds,
                    expires_at_ns,
                } => {
                    let snapshot = ContextSnapshot {
                        context_id,
                        client_id,
                        session_id,
                        messages_json,
                        metadata_json,
                        security_level,
                        created_at_ns,
                        updated_at_ns,
                        ttl_seconds,
                    };
                    Self::upsert_entry(&self.entries, &snapshot, expires_at_ns);
                }
                JournalRecord::Update {
                    context_id,
                    update_type,
                    messages_delta_json,
                    metadata_delta_json,
                    updated_at_ns,
                    expires_at_ns,
                } => {
                    let update = ContextUpdate {
                        context_id,
                        update_type,
                        messages_delta_json,
                        metadata_delta_json,
                        updated_at_ns,
                    };
                    // Delta inválido no journal: loga e segue (mesma regra de linha).
                    if let Err(e) = Self::apply_update_entry(&self.entries, &update, expires_at_ns)
                    {
                        tracing::warn!(line = lineno + 1, error = %e, "journal: update ignorado");
                    }
                }
            }
            applied += 1;
        }
        if applied > 0 {
            tracing::info!(
                applied,
                contexts = self.entries.len(),
                "journal re-executado (replay)"
            );
        }
        self.journaled_ops.store(applied, Ordering::Relaxed);
    }

    /// Upsert espelhando o `ON CONFLICT DO UPDATE` do PostgreSQL: em conflito,
    /// atualiza os campos mutáveis e preserva `client_id`/`session_id`/
    /// `created_at_ns` do primeiro insert.
    fn upsert_entry(
        entries: &DashMap<String, ContextEntry, ahash::RandomState>,
        snapshot: &ContextSnapshot,
        expires_at_ns: u64,
    ) {
        match entries.entry(snapshot.context_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut occ) => {
                let entry = occ.get_mut();
                entry.snapshot.messages_json = snapshot.messages_json.clone();
                entry.snapshot.metadata_json = snapshot.metadata_json.clone();
                entry.snapshot.security_level = snapshot.security_level;
                entry.snapshot.updated_at_ns = snapshot.updated_at_ns;
                entry.snapshot.ttl_seconds = snapshot.ttl_seconds;
                entry.expires_at_ns = expires_at_ns;
            }
            dashmap::mapref::entry::Entry::Vacant(vac) => {
                vac.insert(ContextEntry {
                    snapshot: snapshot.clone(),
                    expires_at_ns,
                });
            }
        }
    }

    /// Aplica um update sobre o mapa (criando o contexto se não existir).
    /// O shard lock do `entry()` cobre leitura+merge+escrita: se o delta for
    /// inválido, nada é inserido (no Python, a exceção aborta antes do `save`).
    /// Núcleo comum de `apply_update` e do replay do journal.
    fn apply_update_entry(
        entries: &DashMap<String, ContextEntry, ahash::RandomState>,
        update: &ContextUpdate,
        expires_at_ns: u64,
    ) -> Result<(), StoreError> {
        // Nota: `metadata_delta_json` existe no contrato IDL mas o
        // `PostgresContextStore.apply_update` (Python) o ignora — não há merge
        // de metadata. Implementação futura: se o Python passar a aplicar,
        // adicionar merge de metadata aqui (similar ao messages_delta_json).
        match entries.entry(update.context_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut occ) => {
                let entry = occ.get_mut();
                let new_messages = apply_messages_delta(
                    &update.context_id,
                    &entry.snapshot.messages_json,
                    update,
                )?;
                entry.snapshot.messages_json = new_messages;
                entry.snapshot.updated_at_ns = update.updated_at_ns;
                entry.expires_at_ns = expires_at_ns;
            }
            dashmap::mapref::entry::Entry::Vacant(vac) => {
                let new_messages = apply_messages_delta(&update.context_id, "[]", update)?;
                let mut snapshot = snapshot_from_update(update, now_ns());
                snapshot.messages_json = new_messages;
                vac.insert(ContextEntry {
                    snapshot,
                    expires_at_ns,
                });
            }
        }
        Ok(())
    }

    /// Grava o registro no journal (write-ahead) e faz flush.
    async fn journal_append(&self, record: &JournalRecord) -> Result<(), StoreError> {
        if let Some(journal) = &self.journal {
            let mut line = serde_json::to_string(record).map_err(StoreError::Json)?;
            line.push('\n');
            let mut file = journal.lock().await;
            file.write_all(line.as_bytes())
                .await
                .map_err(StoreError::Io)?;
            file.flush().await.map_err(StoreError::Io)?;
            self.journaled_ops.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Quantidade de contextos armazenados (inclui expirados ainda não varridos).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` se o store está vazio.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `true` se o contexto existe (não filtra expirados — paridade com `get`).
    pub fn contains(&self, context_id: &str) -> bool {
        self.entries.contains_key(context_id)
    }

    /// Quantidade de mensagens do contexto (`messages_json` parseado), ou
    /// `None` se o contexto não existe / o JSON não é array. Diagnóstico.
    pub fn messages_len(&self, context_id: &str) -> Option<usize> {
        let entry = self.entries.get(context_id)?;
        let v: serde_json::Value = serde_json::from_str(&entry.snapshot.messages_json).ok()?;
        v.as_array().map(Vec::len)
    }

    /// Caminho do journal (se houver).
    pub fn journal_path(&self) -> Option<&Path> {
        self.journal_path.as_deref()
    }

    /// Quantas operações foram persistidas/re-executadas (diagnóstico).
    pub fn journaled_ops(&self) -> u64 {
        self.journaled_ops.load(Ordering::Relaxed)
    }
}

impl ContextStore for LocalContextStore {
    async fn put_snapshot(&self, snapshot: &ContextSnapshot) -> Result<(), StoreError> {
        let expires_at_ns = expires_from_now(snapshot.ttl_seconds);
        let _write = self.write_lock.lock().await;
        self.journal_append(&JournalRecord::put(snapshot, expires_at_ns))
            .await?;
        Self::upsert_entry(&self.entries, snapshot, expires_at_ns);
        tracing::debug!(context_id = %snapshot.context_id, "snapshot persistido");
        Ok(())
    }

    async fn apply_update(&self, update: &ContextUpdate) -> Result<(), StoreError> {
        let _write = self.write_lock.lock().await;
        // O TTL usado na expiração é o do contexto (default 3600 se novo) —
        // equivalente ao `NOW() + INTERVAL * ttl_seconds` do `save()` Python,
        // que relê o snapshot antes de salvar. Lido sob o write lock.
        let ttl = self
            .entries
            .get(&update.context_id)
            .map(|e| e.snapshot.ttl_seconds)
            .unwrap_or(DEFAULT_TTL_SECONDS);
        let expires_at_ns = expires_from_now(ttl);
        self.journal_append(&JournalRecord::update(update, expires_at_ns))
            .await?;
        Self::apply_update_entry(&self.entries, update, expires_at_ns)?;
        tracing::debug!(
            context_id = %update.context_id,
            update_type = update.update_type,
            "update aplicado"
        );
        Ok(())
    }

    async fn get(&self, context_id: &str) -> Result<Option<ContextSnapshot>, StoreError> {
        Ok(self.entries.get(context_id).map(|e| e.snapshot.clone()))
    }

    async fn list_sessions(&self) -> Result<Vec<String>, StoreError> {
        let mut sessions: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| {
                let s = e.snapshot.session_id.clone();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        sessions.sort();
        Ok(sessions)
    }

    async fn expire_ttl(&self) -> Result<u64, StoreError> {
        let now = now_ns();
        let before = self.entries.len();
        self.entries.retain(|_, e| e.expires_at_ns > now);
        let removed = before - self.entries.len();
        if removed > 0 {
            tracing::info!(removed, "contextos expirados removidos");
        }
        Ok(removed as u64)
    }
}
