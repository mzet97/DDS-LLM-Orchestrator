# Evidências T-900-M1-03 — WorkflowSpace por composição

Data: 2026-10-17 (UTC). Base do M1-01/02 aplicada; sem commit.

## Entregas

| Arquivo | Papel |
|---|---|
| `crates/dds-dataspace/src/workflow_space.rs` | Trait `WorkflowSpaceApi` (5 tópicos, composição — `DataSpaceApi` intocada), erros tipados (`CapacityFull`/`InvalidSchemaVersion`/`Stale`/`Conflict`/`CacheBudgetExceeded`), `WorkflowCaches` com admissão compartilhada, `InMemoryWorkflowSpace` (snapshot/restore) e `DdsWorkflowSpace` (feature `dds`) |
| `crates/dds-dataspace/src/qos.rs` | `profiles::workflow`: Reliable(1s)/TransientLocal/KeepLast(1)/Shared/ResourceLimits(N,N,1); `capabilities()` com Deadline 10s + Liveliness ManualByTopic 10s (contracts.md) |
| `crates/dds-dataspace/tests/workflow_space.rs` | 9 testes in-memory (aceite completo) |
| `crates/dds-dataspace/tests/workflow_space_dds.rs` | Smoke DDS real: 2 participantes, publicação/leitura cross-participant + fail-closed (domínio 209) |

## Aceite — "capacidade cheia, versão inválida e lag → erro tipado; snapshots/replay sem perda silenciosa"

- Capacidade: nova chave acima de `max_instances` → `CapacityFull` (substituir chave existente não conta); orçamento de bytes (64 MiB default) → `CacheBudgetExceeded`.
- Versão: `schema_version != 1` → `InvalidSchemaVersion` nos 5 tópicos; NADA é aceito/publicado.
- Lag: `state_revision`/`announcement_revision`/`intent_revision` regressivas → `Stale` (com current/received).
- Conflito: mesma revisão com payload/conteúdo diferente → `Conflict` (Intent/Decision/Plan/State imutáveis por revisão).
- Replay idêntico (at-least-once): aceito, sem instância nova.
- Snapshot/restore: counts 1:1 entre espaços; restore divergente sobre estado existente → erro tipado, sem sobrescrever silencioso.
- Streams: só entregam amostras ACEITAS pela admissão (rejeitada não circula).

## Verificações (2026-10-17)

| Comando | Resultado |
|---|---|
| `cargo test -p dds-dataspace --locked --offline` | **30/0** (9 novos + 1 unit) |
| `cargo test -p dds-dataspace --features dds --locked --offline` | **56/0** (inclui smoke cross-participante e regressão existente) |
| `cargo clippy -p dds-datas --all-targets [--features dds] -- -D warnings` | exit 0 (ambos) |
| `cargo fmt --all -- --check` | exit 0 |

## Decisões

- Escrita DDS valida ANTES de publicar (fail-closed: amostra inválida não circula na malha) — smoke confirma `req-bad` invisível ao leitor.
- `subscribe_*` DDS usa `take_aiter` (reader por stream, padrão pré-T-617): consumidores da fase 900 são poucos; migração para `SharedWaitSet` fica para M5 se medição pedir (divergência documentada no código).
- Bounds de string: o cache de aplicação não revalida bounds por campo no lado DDS (R-INTEROP: `DdsString` sem bound no gerador Rust); limites de instância/bytes e validações de versão/revisão aplicam.
