# Tasks 100 — Agente

- [x] **T-201 · Engine trait + MockEngine** (REQ-203)
  `trait Engine { async fn infer_stream(&self, req) -> impl Stream<Chunk> }`; `MockEngine`.
  *Aceite:* teste do MockEngine emite chunks previsíveis.
  **Status:** ✅ 2026-07-18 — `tests/engine.rs` (3 testes verdes).

- [x] **T-202 · Claim loop + seleção** (REQ-201, REQ-207)
  Assinar Tasks (stream), filtrar por especialização/`target_agent`, escolher, ASSIGNED.
  *Aceite:* PENDING compatível assumida; incompatível ignorada (teste com stub).
  **Status:** ✅ 2026-07-18 — `AgentDds::run`; E2E 10/10 tasks claimed (teste com stub Python no A/B).

- [x] **T-203 · Confirmação de ownership** (REQ-202)
  Readback pós-write; ceder se outro detém.
  *Aceite:* disputa 2 agentes → exatamente 1 executa (0 execução dupla).
  **Status:** ✅ 2026-07-18 — readback via **`read_task_mesh`** (estado ARBITRADO do RHC;
  empate de strength decide por menor GUID, mesh-wide). Achado central: write-through no
  cache do DataSpace e "maior timestamp" no upsert tornavam o readback inútil —
  removido write-through; upsert = anti-regressão + last-write-wins (== Python).
  A/B 1 Rust + 1 Python, 100 tasks: **0 execução dupla**.

- [x] **T-204 · Pool MPMC de writers de TaskOutput** (REQ-204)
  Pool `crossbeam` bounded para chunks; sem thread única.
  *Aceite:* chunks publicados pelo pool; backpressure documentada.
  **Status:** ✅ 2026-07-18 — `WriterPool` (dataspace); E2E 30/30 chunks íntegros.

- [x] **T-205 · DdsEngine: ponte ao llama-server C++** (REQ-204)
  `LLM.InferenceRequest` → `LLM.InferenceResult/Error` por request_id; timeout por deadline.
  *Aceite:* resposta real recebida e correlacionada.
  **Status:** ✅ 2026-07-18 — `tests/engine_dds.rs` contra llama-server real:
  "Hello", 2 chunks, correlação e timeout OK.

- [x] **T-206 · Heartbeat dedicado** (REQ-205)
  tokio interval 5 s; `AgentState` ManualByTopic (lease 10 s); uptime real.
  *Aceite:* heartbeat no AgentRegistry; não congela sob inferência.
  **Status:** ✅ 2026-07-18 — `spawn_heartbeat`; E2E: `completed_total=10`, `uptime>0`.

- [x] **T-207 · Coexistência A/B: 1 Rust + N Python** (REQ-207)
  100 tasks; ownership arbitra; 0 execução dupla.
  *Aceite:* união cobre as tasks; interseção vazia.
  **Status:** ✅ 2026-07-18 — `scripts/ab_coexistence.sh`: 100/100 DONE, interseção 0;
  vencedor da rodada por GUID (documentado no REPORT §achados).

- [x] **T-208 · Bench + REPORT** (Roadmap)
  Throughput/latência vs agente Python.
  *Aceite:* números medidos + REPORT.
  **Status:** ✅ 2026-07-18 — claim loop **4,02 tasks/s** (confirm sequencial 250 ms;
  alavanca: confirmações paralelas). REPORT.md escrito.

## Gate de saída (Fase 1)
1 agente Rust + N Python coexistem ✓ · zero execução dupla ✓ · paridade de
comportamento (claim/engine/heartbeat) ✓ · números medidos ✓ · REPORT ✓
