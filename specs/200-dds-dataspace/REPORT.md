# Report 200 — Camada DDS (`dds-dataspace`)

**Data:** 2026-07-17 · **Status:** ✅ Concluída (gate da Fase 2 atingido)
**Aceite do gate:** API async estável ✓ · contract tests A/B verdes ✓ · orçamento de propagação ✓

---

## O que foi construído

A camada DDS de coordenação em Rust, substituindo `src/orchestrator/dds_backend/` (~3,4k LOC Python):

| Módulo | Conteúdo |
|---|---|
| `src/api.rs` | trait `DataSpaceApi` (async, Send+Sync) — tasks/agents/outputs + subscribe + shutdown |
| `src/in_memory.rs` | `InMemoryDataSpace` (mock com DashMap + broadcast, p/ testes) |
| `src/lib.rs` | `DataSpace` real: participant + 3 tópicos canônicos + writers/readers, drop ordenado |
| `src/qos.rs` | Perfis QoS por tópico **espelhando o SEDP do Python** (ownership/strength, reliability 10s, liveliness por tópico, deadline, latency, tprio) |
| `src/cache.rs` | `TopicCaches`: `Arc<T>` imutável + DashMap sharded; **regressão de status bloqueada por construção** (upsert monotônico); dedup de outputs por seq_num |
| `src/writer_pool.rs` | Pool MPMC (crossbeam bounded, K workers) + backpressure fail-fast |
| `src/monitor.rs` | `QosMonitor` com listeners nativos (`on_liveliness_changed`, `on_requested_deadline_missed`) → eventos por broadcast |

Streams por evento (`stream_tasks`/`stream_agent_states`/`stream_task_outputs`): cada chamada
cria um reader dedicado ('static), alimenta os caches e entrega `Arc<T>` — sem polling.

## Números medidos (neste host, `--features dds`, domínios isolados)

| Medida | Valor | Referência |
|---|---:|---|
| **Propagação de estado (write→assinante)** | **p50 0,052 ms · mean 0,054 · p95 0,062 · p99 0,077 ms** (n=500) | orçamento p99 < 5 ms → **65× abaixo** |
| Wakeup de stream (T-304) | p50 0,059–0,064 ms | evento WaitSet, sem busy-wait |
| Throughput pool de writers (K=4) | **88.752 tasks/s** (5k tasks em 56 ms, 0 falhas) | thread única Python = gargalo |
| Backpressure | 12/16 rejeitados com fila cheia (fail-fast) | política documentada |
| Baseline Python (spike benchmark, mesmo host) | p50 19,068 ms RTT | **propagação Rust ~365× mais rápida no p50** |

## Testes (13 verdes)

| Arquivo | Testes |
|---|---|
| `tests/contract.rs` | bateria `DataSpaceApi` **A/B: mock + DDS real** (tasks/agents/outputs, subscribe wakeup, shutdown) |
| `tests/lifecycle.rs` | sobe/derruba sem vazar; 2 instâncias em domínios distintos |
| `tests/cache.rs` | upsert sem regressão, dedup outputs, stress 1600 tasks/16 threads, 200 corridas mesmo id |
| `tests/streams.rs` | wakeup por amostra + bench de propagação (500 amostras) |
| `tests/writer_pool.rs` | throughput 5k real + backpressure fail-fast |
| `tests/monitor.rs` | liveliness join/leave (lease 2s, saída à la SIGKILL) + deadline missed (1s) |

## Correções na crate `cyclonedds` feitas nesta fase (necessárias)

1. **`async.rs`: UB com tipos contendo `String`** — os caminhos async (`take_async`, `*_aiter`)
   faziam `std::ptr::read` na amostra nativa (layout C: `char*` de 8B) reinterpretando como
   struct Rust (`String` de 24B) — lia len/cap como lixo e liberava ponteiros arbitrários
   (double-free/heap corruption) após `dds_return_loan`. Substituído por `T::clone_out`
   (como o caminho síncrono). Regressão coberta em `dds-contract/tests/async_soundness.rs`.
2. **`Topic<T>` não era `Send`/`Sync`** (`Rc` interno) — impossibilitava o uso em runtime
   multi-thread. Trocado por `Arc` + `unsafe impl Send/Sync` no holder (imutável; ponteiros
   estáveis; C copia na criação). Pré-requisito para `DataSpaceApi: Send + Sync`.
3. **Caveat documentado:** `Listener` deve sobreviver à entidade (o C chama via ponteiro;
   dropar cedo = use-after-free/SIGSEGV — encontrado no teste do monitor).
4. **Nota de comportamento:** writer `ManualByTopic` precisa de `assert_liveliness()` explícito
   para o evento inicial de alive; saída limpa (dispose) **não** gera `not_alive` — para
   detecção de morte à la SIGKILL, o caminho é a expiração do lease.

## Desvios e decisões

- A bateria de contrato tolera o histórico TransientLocal no subscribe (mock não tem) —
  diferença semântica legítima A/B documentada no teste.
- Streams criam reader dedicado por chamada ('static, sem corrida de `take` entre assinantes).
- `WriterPool` com writers dedicados (mesmos perfis/strength) — `DataWriter` é thread-safe
  para `write` concorrente; backpressure = fail-fast (documentado).

## Handoff (para Fase 1/3)

- O `agent` (T-205) e o `orchestrator` (T-401) consomem `DataSpaceApi` — mock nos testes,
  `DataSpace` real com `--features dds`.
- Perfis QoS de produção: usar `dds_dataspace::qos::profiles` (espelho fiel do Python).
- CFT da crate é writer-side (não-SQL) — não usado nesta fase; avaliar ao portar o
  `dds_backend` que usa `ContentFilteredTopic` SQL.
- Verificação contínua: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`.
