# Plan 200 — Camada DDS (como)

## Módulos (crate `dds-dataspace`)
```
src/lib.rs          # DataSpace (fachada async) + trait DataSpaceApi (mock e real)
src/readers.rs      # REQ-302/303: streams por tópico (WaitSet/take_aiter, loans)
src/writers.rs      # REQ-305: pool MPMC + backpressure
src/caches.rs       # REQ-304/306: dashmap<String, Arc<T>> por tópico
src/liveliness.rs   # REQ-307: listener nativo + fallback por idade
src/qos_monitor.rs  # REQ-308: deadline missed + reliability gaps
src/mock.rs         # REQ-309: InMemoryDataSpace (mesma trait)
```

## Decisões técnicas
- **`trait DataSpaceApi`** implementada por `DdsDataSpace` (real, feature `dds`) e
  `InMemoryDataSpace` (mock) → contract tests A/B (REQ-309) rodam contra ambos.
- **Streams:** cada reader vira `impl Stream<Item = Arc<T>>` via `take_aiter`. O control
  loop consome por `select!`/`StreamExt`. Housekeeping vira `tokio::time::interval`.
- **Zero-copy:** onde a crate expõe `take_loan`, entregar o loan até a fronteira do cache;
  copiar só ao materializar em `Arc<T>` (medir o ganho).
- **Writers:** `crossbeam-channel` bounded (backpressure); K tarefas de write consumindo.
  Política de sobrecarga: bloquear com timeout ou dropar-e-logar (registrar decisão).
- **Task imutável:** o cache guarda `Arc<Task>`; mutação = `Arc::new(task_atualizado)` +
  `insert` (replace). Elimina a corrida C1 e as guardas `_is_state_regression`/
  `_merge_task_timestamps` do Python.
- **Ownership:** strength por papel vinda de `dds-contract::roles`.
- **Liveliness:** `ListenerBuilder::on_liveliness_changed` da crate; fallback por
  `last_update` como telemetria.

## Paridade (Python → Rust)
| Python (`dds_backend`) | Rust | Nota |
|---|---|---|
| `_poll_loop` (16 refreshers) | streams por tópico + `select!` | sem polling |
| `_write_queue` + 1 thread | `writers` pool MPMC | sem serialização |
| `_tasks_cache` + RLock | `caches` dashmap | sem lock global |
| `_is_state_regression`/merge | `Arc<Task>` imutável | corrida some |
| `_check_agent_liveliness` | listener nativo + fallback | sem deadlock de GIL |
| `apply_qos_profile` (mutáveis) | `set_online_knobs` | só TransportPriority/LatencyBudget/OwnershipStrength |

## Teste
- Contract tests (REQ-309) em `tests/contract.rs` parametrizados por `impl DataSpaceApi`.
- Concorrência (REQ-304): loom ou stress com N threads.
- Bench de propagação (REQ-310) vs Python.

## Orçamento
Propagação < 5 ms p99; sem alocação por amostra no caminho de TaskOutput.
