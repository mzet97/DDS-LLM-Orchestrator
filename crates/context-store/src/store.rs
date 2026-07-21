//! Trait `ContextStore` e a semântica de merge/TTL portada de
//! `src/orchestrator/context_store/postgres_store.py` (Python).
//!
//! ## Semântica portada (paridade com `PostgresContextStore`)
//!
//! - **`save()` → `put_snapshot`**: upsert por `context_id`. Em conflito,
//!   atualiza apenas `messages_json`, `metadata_json`, `security_level`,
//!   `updated_at_ns`, `ttl_seconds` (e recalcula a expiração); **preserva**
//!   `client_id`, `session_id` e `created_at_ns` do primeiro insert
//!   (espelha o `ON CONFLICT DO UPDATE` do PostgreSQL).
//! - **`apply_update()`**: se o contexto não existe, cria um vazio com os
//!   defaults de `models.py` (`messages_json="[]"`, `metadata_json="{}"`,
//!   `security_level=0`, `ttl_seconds=3600`). Depois aplica o delta de
//!   mensagens conforme `update_type`:
//!   - `0` (APPEND): faz parse de `messages_json` e de `messages_delta_json`
//!     como arrays JSON e estende o histórico (`existing.extend(delta)`);
//!   - `1` (REPLACE): `messages_json = messages_delta_json` (verbatim);
//!   - `2` (CLEAR): `messages_json = "[]"`;
//!   - outro: não toca nas mensagens (o Python ignora silenciosamente).
//!     Ao final, `updated_at_ns = update.updated_at_ns` e persiste.
//!     **`metadata_delta_json` NÃO é aplicado** — existe no contrato IDL, mas o
//!     `PostgresContextStore.apply_update` o ignora (ver TODO no código).
//! - **TTL**: a expiração é absoluta (`agora + ttl_seconds`, recomputada a
//!   cada escrita — equivalente ao `expires_at = NOW() + INTERVAL` do
//!   PostgreSQL). `get` **não** filtra expirados (paridade: no Python a
//!   limpeza só acontece em `delete_expired()`); `expire_ttl` remove os
//!   vencidos e retorna quantos foram removidos (`rowcount`).

use dds_contract::generated::dds_llm_orchestrator::{ContextSnapshot, ContextUpdate};

/// `update_type = 0`: anexa `messages_delta_json` (array) ao histórico.
pub const UPDATE_APPEND: i32 = 0;
/// `update_type = 1`: substitui o histórico por `messages_delta_json` (verbatim).
pub const UPDATE_REPLACE: i32 = 1;
/// `update_type = 2`: limpa o histórico (`"[]"`).
pub const UPDATE_CLEAR: i32 = 2;

/// TTL default de um contexto criado a partir de um update
/// (igual a `models.py`: `ttl_seconds: int = 3600`).
pub const DEFAULT_TTL_SECONDS: u32 = 3600;

/// Erros do Context Store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Falha de I/O no journal JSONL (durabilidade do `LocalContextStore`).
    #[error("I/O no journal JSONL: {0}")]
    Io(#[from] std::io::Error),
    /// JSON malformado em `messages_json`/`messages_delta_json`
    /// (equivalente ao `json.JSONDecodeError` do Python).
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    /// APPEND (`update_type=0`) exige que ambos os campos sejam arrays JSON
    /// (no Python, `list.extend` em não-lista falha com `AttributeError`).
    #[error("contexto {context_id}: campo {field} não é um array JSON")]
    NotJsonArray {
        /// Contexto cujo campo não é array.
        context_id: String,
        /// Campo ofensor (`messages_json` ou `messages_delta_json`).
        field: &'static str,
    },
}

/// Interface do Context Store (port do `PostgresContextStore`).
///
/// O store é compartilhado entre tasks (serviço DDS + leitores), então todos
/// os métodos tomam `&self`; a concorrência interna é da implementação
/// (dashmap no `LocalContextStore`).
// Futures são `Send`: as implementações só capturam tipos `Send` por `&self`.
#[allow(async_fn_in_trait)]
pub trait ContextStore: Send + Sync {
    /// Insere ou atualiza um snapshot (upsert — ver semântica no módulo).
    async fn put_snapshot(&self, snapshot: &ContextSnapshot) -> Result<(), StoreError>;

    /// Aplica um `ContextUpdate` ao snapshot existente (criando-o se não houver).
    async fn apply_update(&self, update: &ContextUpdate) -> Result<(), StoreError>;

    /// Lê um snapshot por `context_id`. Não filtra expirados (paridade).
    async fn get(&self, context_id: &str) -> Result<Option<ContextSnapshot>, StoreError>;

    /// Lista os `session_id` distintos presentes no store (índice
    /// `idx_contexts_session` do schema PostgreSQL, em memória).
    async fn list_sessions(&self) -> Result<Vec<String>, StoreError>;

    /// Remove os contextos com expiração vencida (`delete_expired` do Python).
    /// Retorna quantos foram removidos.
    async fn expire_ttl(&self) -> Result<u64, StoreError>;
}

/// Aplica o delta de mensagens conforme `update_type` (núcleo de
/// `PostgresContextStore.apply_update`). Função pura: não persiste nada.
///
/// - APPEND: parse de ambos como array e `extend` (erro `NotJsonArray` se
///   algum não for array — ver [`StoreError`]);
/// - REPLACE: retorna o delta verbatim;
/// - CLEAR: retorna `"[]"`;
/// - desconhecido: retorna `current` inalterado (o Python só faz o bump de
///   `updated_at_ns`; aqui o caller faz o mesmo).
pub(crate) fn apply_messages_delta(
    context_id: &str,
    current: &str,
    update: &ContextUpdate,
) -> Result<String, StoreError> {
    match update.update_type {
        UPDATE_APPEND => {
            let mut existing: serde_json::Value = serde_json::from_str(current)?;
            let delta: serde_json::Value = serde_json::from_str(&update.messages_delta_json)?;
            let arr = existing
                .as_array_mut()
                .ok_or_else(|| StoreError::NotJsonArray {
                    context_id: context_id.to_string(),
                    field: "messages_json",
                })?;
            let d = delta.as_array().ok_or_else(|| StoreError::NotJsonArray {
                context_id: context_id.to_string(),
                field: "messages_delta_json",
            })?;
            arr.extend(d.iter().cloned());
            Ok(serde_json::to_string(&existing)?)
        }
        UPDATE_REPLACE => Ok(update.messages_delta_json.clone()),
        UPDATE_CLEAR => Ok("[]".to_string()),
        other => {
            tracing::warn!(
                context_id,
                update_type = other,
                "update_type desconhecido — mensagens preservadas (paridade Python)"
            );
            Ok(current.to_string())
        }
    }
}

/// Snapshot default criado quando chega um update para contexto inexistente
/// (defaults de `models.py`; `created_at_ns` usa o relógio local, como o
/// `field(default_factory=now_ns)` do dataclass).
pub(crate) fn snapshot_from_update(update: &ContextUpdate, now_ns: u64) -> ContextSnapshot {
    ContextSnapshot {
        context_id: update.context_id.clone(),
        client_id: String::new(),
        session_id: String::new(),
        messages_json: "[]".to_string(),
        metadata_json: "{}".to_string(),
        security_level: 0,
        created_at_ns: now_ns,
        updated_at_ns: update.updated_at_ns,
        ttl_seconds: DEFAULT_TTL_SECONDS,
    }
}

/// Relógio wall-clock em ns (equivale ao `NOW()` do PostgreSQL / `now_ns` do Python).
pub(crate) fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Expiração absoluta a partir de agora (`NOW() + INTERVAL '1 second' * ttl`).
pub(crate) fn expires_from_now(ttl_seconds: u32) -> u64 {
    now_ns().saturating_add(u64::from(ttl_seconds) * 1_000_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(context_id: &str, update_type: i32, delta: &str) -> ContextUpdate {
        ContextUpdate {
            context_id: context_id.to_string(),
            update_type,
            messages_delta_json: delta.to_string(),
            metadata_delta_json: "{}".to_string(),
            updated_at_ns: 42,
        }
    }

    #[test]
    fn append_estende_historico() {
        let u = update("c1", UPDATE_APPEND, r#"[{"role":"user","content":"b"}]"#);
        let merged = apply_messages_delta("c1", r#"[{"role":"user","content":"a"}]"#, &u).unwrap();
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[1]["content"], "b");
    }

    #[test]
    fn append_exige_arrays() {
        let u = update("c1", UPDATE_APPEND, r#"{"a":1}"#);
        let err = apply_messages_delta("c1", "[]", &u).unwrap_err();
        assert!(matches!(
            err,
            StoreError::NotJsonArray {
                field: "messages_delta_json",
                ..
            }
        ));
    }

    #[test]
    fn replace_e_clear() {
        let u = update("c1", UPDATE_REPLACE, r#"[{"role":"system","content":"x"}]"#);
        assert_eq!(
            apply_messages_delta("c1", r#"[{"old":true}]"#, &u).unwrap(),
            u.messages_delta_json
        );
        let u = update("c1", UPDATE_CLEAR, "[]");
        assert_eq!(
            apply_messages_delta("c1", r#"[{"old":true}]"#, &u).unwrap(),
            "[]"
        );
    }

    #[test]
    fn tipo_desconhecido_preserva_mensagens() {
        let u = update("c1", 99, r#"[{"x":1}]"#);
        assert_eq!(
            apply_messages_delta("c1", r#"[{"old":true}]"#, &u).unwrap(),
            r#"[{"old":true}]"#
        );
    }
}
