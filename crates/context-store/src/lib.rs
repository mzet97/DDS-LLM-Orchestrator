//! # context-store
//!
//! Port do componente Python `src/orchestrator/context_store/` para Rust:
//! persiste o contexto conversacional (snapshots e deltas) que circula pelos
//! tópicos DDS `Context.Snapshot` e `Context.Update`.
//!
//! ## Arquitetura
//! - [`ContextStore`]: trait async com a API do `PostgresContextStore` Python
//!   (`put_snapshot` ≈ `save`, `apply_update`, `get`, `list_sessions`,
//!   `expire_ttl` ≈ `delete_expired`). A semântica exata do merge de deltas e
//!   do TTL está documentada em [`store`].
//! - [`LocalContextStore`]: implementação local — `DashMap` em memória +
//!   journal JSONL append-only para durabilidade leve (replay no restart).
//! - [`service::ContextStoreService`] (feature `dds`): assina
//!   `Context.Snapshot`/`Context.Update` via `dds_dataspace::DataSpace` e
//!   alimenta o store (port do `main.py`, que assinava `Context.Update`).
//! - Binário `context-store` (requer `--features dds`): entry point do
//!   serviço, paridade com `context_store/main.py`.
//!
//! ## TODO(follow-up): `PostgresContextStore`
//! O backend PostgreSQL (psycopg/AsyncConnectionPool, tabela `contexts` com
//! índices em `session_id`/`expires_at`) fica como trabalho futuro sobre a
//! **mesma trait** [`ContextStore`]: basta uma struct `PostgresContextStore`
//! (sqlx ou tokio-postgres — a decidir; **não** adicionar diesel/sqlx antes
//! de validado com o líder da migração) implementando os 5 métodos com o SQL
//! de `postgres_store.py`. O serviço DDS e o binário não mudam — recebem
//! qualquer `S: ContextStore`.

pub mod local;
pub mod store;

#[cfg(feature = "dds")]
pub mod service;

pub use local::LocalContextStore;
pub use store::{
    ContextStore, StoreError, DEFAULT_TTL_SECONDS, UPDATE_APPEND, UPDATE_CLEAR, UPDATE_REPLACE,
};

#[cfg(feature = "dds")]
pub use service::ContextStoreService;
