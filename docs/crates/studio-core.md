# `studio-core` — domínio puro do catálogo

> Crate: `src/rust/crates/studio-core/` · Sem IO/rede (só `thiserror` + `serde`) ·
> GUI, nó e autoridade compartilham a mesma semântica testada.

## Propósito

Domínio administrativo puro do Studio: revisões condicionais (REQ-800), gerações
de intenção (REQ-801) e catálogo autoritativo em memória com snapshot/cursor,
log de eventos com retenção e tombstones.

## API pública

- **`RevisionGuard`**: aceita só `base == current`, avança exatamente 1,
  `Overflow` em saturação. Erros `RevisionError::{Stale, Overflow}`.
- **`GenerationGuard`**: aceita só geração estritamente maior.
  Erro `GenerationError::Stale`.
- **`Catalog`**: `publish(id, base, value, generation)` (criação rev 0;
  atualização via guards; tombstone bloqueia), `delete` (tombstone), `snapshot`
  (itens ordenados + cursor), `events_since` (contíguos; expira fora de
  `RETENTION=64`). Erros `CatalogError::{Conflict, StaleGeneration, Tombstoned,
  CursorExpired}`.
- Tipos: `catalog::{DefinitionId, Cursor, Event, EventKind, ItemView, Snapshot}`,
  `revision::{Revision, Generation, ...}`. Raiz reexporta `Catalog`.

## Testes

```bash
cd src/rust
cargo test -p studio-core   # guards, tombstone, cursor (catalog.rs / revision.rs)
```

Sem binários, features, env vars ou flags — por desenho.
