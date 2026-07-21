# Tasks 200 — Camada DDS

- [x] **T-301 · trait DataSpaceApi + InMemory mock** (REQ-309)
  Definir a trait (write/read/subscribe por tópico) e o `InMemoryDataSpace`.
  *Aceite:* mock passa uma bateria mínima de contract test.
  **Status:** ✅ 2026-07-17 — `tests/contract.rs`: bateria completa (tasks/agents/outputs,
  subscribe wakeup, shutdown) verde contra o mock.

- [x] **T-302 · Ciclo de vida do DataSpace real** (REQ-301) `[--features dds]`
  participant/pub/sub/tópicos/readers/writers com QoS de `dds-contract`.
  *Aceite:* sobe e derruba sem vazar; teste smoke.
  **Status:** ✅ 2026-07-17 — `src/qos.rs` (perfis espelhando o SEDP Python),
  `DataSpace::new` (participant + 3 tópicos + writers/readers, drop ordenado);
  `tests/lifecycle.rs`: smoke write/read-back/shutdown + 2 instâncias em domínios distintos.

- [x] **T-303 · Caches concorrentes (Arc + dashmap)** (REQ-304, REQ-306)
  Caches por tópico; Task imutável (`Arc<Task>`); sem guardas anti-regressão.
  *Aceite:* teste concorrente sem corrupção; disputa sem regressão.
  **Status:** ✅ 2026-07-17 — `src/cache.rs`: upsert monotônico (regressão bloqueada por
  construção), dedup de outputs por seq_num; 4 testes verdes (stress 1600 tasks/16 threads,
  200 corridas mesmo id sem regressão).

- [x] **T-304 · Streams por evento (WaitSet/aiter)** (REQ-302, REQ-303)
  Readers como `Stream`; loans no hot path onde possível.
  *Aceite:* wakeup por amostra sem busy-wait; latência de wakeup medida.
  **Status:** ✅ 2026-07-17 — `subscribe_tasks/agent_states/task_outputs` sobre
  `take_aiter` (após fix de soundness em `cyclonedds/src/async.rs`: `ptr::read`→`clone_out`,
  era UB com tipos contendo String); alimentam os caches; **wakeup p50=0,059 ms, p99=0,344 ms**.

- [x] **T-305 · Pool de writers + backpressure** (REQ-305)
  MPMC bounded; K writers; política de sobrecarga documentada.
  *Aceite:* streaming sustenta a taxa; backpressure sob sobrecarga.
  **Status:** ✅ 2026-07-17 — `src/writer_pool.rs`: crossbeam bounded + K workers,
  **88.752 tasks/s** (5k em 56 ms); backpressure fail-fast testado (12/16 rejeitados).

- [x] **T-306 · Liveliness nativa + monitor de QoS** (REQ-307, REQ-308)
  Listener `on_liveliness_changed` + fallback; deadline missed + reliability gaps.
  *Aceite:* callback dispara em SIGKILL (se multi-proc); gap/deadline detectados.
  **Status:** ✅ 2026-07-17 — `src/monitor.rs`: eventos LivelinessChanged (join/leave,
  lease 2s, saída à la SIGKILL via `mem::forget`) e DeadlineMissed (1s). Caveat:
  `Listener` deve sobreviver à entidade; writer ManualByTopic precisa `assert_liveliness()`
  p/ o alive inicial; dispose limpo ≠ lease expirado.

- [x] **T-307 · Contract tests A/B (mock vs DDS)** (REQ-309)
  Mesma bateria parametrizada por `impl DataSpaceApi`.
  *Aceite:* passa nos dois backends.
  **Status:** ✅ 2026-07-17 — `tests/contract.rs::contract_battery` verde em
  `InMemoryDataSpace` e `DataSpace` (tolerância ao histórico TransientLocal documentada).

- [x] **T-308 · Bench de propagação + REPORT** (REQ-310, gate)
  Medir propagação de estado vs Python; escrever REPORT.
  *Aceite:* < 5 ms p99 (ou justificar); clippy/fmt ✓.
  **Status:** ✅ 2026-07-17 — **p50 0,052 / mean 0,054 / p95 0,062 / p99 0,077 ms** (n=500,
  65× abaixo do orçamento; Python ~19 ms p50 no spike → ~365×). REPORT.md escrito.

## Gate de saída (Fase 2)
API async estável · contract tests A/B verdes · orçamento de propagação atingido · REPORT.
