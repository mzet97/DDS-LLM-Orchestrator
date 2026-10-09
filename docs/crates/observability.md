# `observability` — eventos, QoS store, traces e trackers

> Crate: `src/rust/crates/observability/` (10ª do workspace) · Porte de
> `observability/`, `qos_collector/`, `trace_collector/`, `metrics/` (Python) ·
> Binário `observability-collector`.

## Propósito

Stack de observabilidade do orquestrador: schema unificado de eventos, sink
JSONL, store QoS em memória, coletores QoS/Trace e trackers (tokens/RTT/
erros/custo).

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p observability --features dds -- \
  --dds-domain 0 [--output-dir ./observability_output]
# Flags: --dds-domain (0), --output-dir. Parsing tolerante. Sem `dds`: stub e sai.
```

O serviço: sink `events.jsonl` + `TraceCollector` + `DataSpace::new(domain, 200)` +
ingestão, flush periódico 30 s + SIGINT gracioso. Env: `RUST_LOG` (`info`).

## Componentes

- **`EventType`** (`events.rs`): paridade com o IntEnum Python (`QosViolation=40`;
  extensões Rust `QosMetric=41`, `QosDiscovery=42`); `ObservabilityEvent` com
  defaults compatíveis ao `query()` Python, id 12-hex.
- **`EventSink`** (trait `emit`/`query`/`flush`) + **`FileEventSink`**: buffer +
  flush a cada 50 eventos, contadores atômicos; `query` faz flush antes de ler e
  ignora linhas truncadas.
- **`QosStore`**: 3 `DashMap` (ahash) p/ métricas, violações, discoveries;
  `upsert_*`, `all_*`, `*_count`, `clear`. Substitui Postgres+`schema.sql`.
- **`QosCollector`**: ingestão **dual-write** (store + evento no sink, sem janela
  de perda); `spawn_ingestion` sob `dds` com flush periódico.
- **`TraceCollector`**: agrega `Execution.Trace` por `trace_id`; `flush`
  **drain-and-write** (sem duplicar entre flushes; em erro reinsere best-effort);
  grava `traces.jsonl`.
- **Trackers**: `TokenCounter` (prompt/completion/count), `RttTracker` (média +
  EMA α=0,1), `ErrorTracker` (8 categorias 0–7), `CostTracker` (micro-USD atômico).
- **`dds.rs`**: `spawn_ingestion` (QoS + traces sobre o mesmo `DataSpace`).

## Testes

```bash
cd src/rust
cargo test -p observability            # 19 unitários inline
CYCLONEDDS_STATIC=1 cargo test -p observability --features dds
```

## Limites

- **Sem Postgres**: `QosStore` é DashMap + snapshot JSONL; sem persistência
  relacional nem queries históricas além de `query()` linear em arquivo.
- **Sem diretório `tests/`**: só unitários inline; `dds.rs` (ingestão real) e o
  loop/SIGINT do `main.rs` sem testes.
- `QOS_METRIC`/`QOS_DISCOVERY` (41/42) não existem no IntEnum Python — interop
  JSONL só num sentido até o Python conhecê-los.
- `RttTracker` EMA fixa; `ErrorTracker` sem enum nomeado.
