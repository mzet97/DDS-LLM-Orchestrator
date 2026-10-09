# `context-store` — persistência de contexto conversacional

> Crate: `src/rust/crates/context-store/` · Port de `src/orchestrator/context_store/` ·
> Binário `context-store` (exige `dds`).

## Propósito

Persiste o contexto conversacional que circula nos tópicos DDS `Context.Snapshot`
/ `Context.Update`. API = trait async `ContextStore` (port do `PostgresContextStore`):
`put_snapshot` (≈`save`, upsert), `apply_update` (merge de delta), `get`,
`list_sessions`, `expire_ttl` (≈`delete_expired`). Implementação atual:
`LocalContextStore` (DashMap em memória + journal JSONL append-only, replay no boot).
Backend PostgreSQL é follow-up futuro sobre a mesma trait.

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p context-store --features dds -- \
  --dds-domain 0 --data-file context_store.jsonl
# Flags: --dds-domain (0), --data-file (context_store.jsonl), --log-level (INFO). Encerra em SIGINT/SIGTERM.
```

Env: `RUST_LOG` (precede `--log-level`); `CYCLONEDDS_STATIC=1` no build DDS.

## Componentes

- **`store.rs`**: trait `ContextStore` (5 métodos async, `&self`, Send+Sync);
  `apply_messages_delta` pura (APPEND/REPLACE/CLEAR; desconhecido→preserva);
  consts `UPDATE_APPEND=0`, `UPDATE_REPLACE=1`, `UPDATE_CLEAR=2`,
  `DEFAULT_TTL_SECONDS=3600`; `StoreError` (Io/Json/NotJsonArray).
- **`LocalContextStore`** (`local.rs`): DashMap+ahash, `write_lock`, journal
  opcional, `journaled_ops`. `open(path)`: replay tolerante (linhas corrompidas
  puladas) + handle de append. `upsert_entry`: semântica ON CONFLICT (preserva
  client/session/created). `expire_ttl`: grava tombstone `Expire` no journal
  antes de remover (REQ/T-820-15).
- **`ContextStoreService<S>`** (`service.rs`): consome `stream_context_snapshots/
  updates`, varredura TTL a cada 60 s.

API (re-exports): `LocalContextStore::{in_memory, open, len, is_empty, contains,
messages_len, journal_path, journaled_ops}`, trait `ContextStore`, `StoreError`,
consts, `ContextStoreService::{new, dataspace, store, run}` (feature `dds`).

## Testes

```bash
cd src/rust
cargo test -p context-store
CYCLONEDDS_STATIC=1 cargo test -p context-store --features dds -- --test-threads=1
# tests/store.rs (429 L: put/get, merge, TTL, journal) + tests/dds_ingest.rs (dom 91, ciclo fim-a-fim)
```

## Limites

- Sem backend PostgreSQL (só `LocalContextStore`).
- `metadata_delta_json` ignorado no `apply_update` (paridade com Python).
- `get` não filtra expirados (paridade intencional); expiração só via
  `expire_ttl`/varredura.
- Journal append-only sem compactação (replay O(operações)); sem fsync por
  escrita (só `flush` — "durabilidade leve").
