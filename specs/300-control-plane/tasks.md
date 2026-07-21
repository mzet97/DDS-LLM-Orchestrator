# Tasks 300 — Control plane

### orchestrator
- [x] **T-401 · API axum + enfileiramento** (REQ-401) — *Aceite:* POST aceita e enfileira.
  **Status:** ✅ 2026-07-18 — POST publica `Task` no tópico com **strength de cliente (10)**
  (descoberta: publicar com 200 impedia o claim dos agentes); E2E valida.
- [x] **T-402 · Scheduler (heap de prioridade)** (REQ-402) — *Aceite:* ordem correta.
  **Status:** ✅ 2026-07-18 — teste de ordem (prioridade, depois idade).
- [x] **T-403 · Registry (liveliness → reassign)** (REQ-403) — *Aceite:* agente morto reatribui.
  **Status:** ✅ 2026-07-18 — reaper por staleness de heartbeat; SIGKILL simulado
  (`mem::forget`) → task volta PENDING com retry+1.
- [x] **T-404 · Selector/Dispatcher** (REQ-404) — *Aceite:* roteamento TEXT/VISION.
  **Status:** ✅ 2026-07-18 — testes de especialização/least-loaded/indisponível.
- [x] **T-405 · Loop de controle + NFCM + knobs online** (REQ-405) — *Aceite:* degradado→Failover; trace `qos_decision`.
  **Status:** ✅ 2026-07-18 — degradado → QoS_Failover; `set_qos` aplica
  TransportPriority+OwnershipStrength; trace a cada período. **Limitação medida:**
  `latency_budget` não mutável em runtime neste CycloneDDS (OUT_OF_MEMORY) — omitido do
  set quente; reportado.
- [x] **T-406 · State machine** (REQ-406) — *Aceite:* transição inválida rejeitada.
  **Status:** ✅ já existia (4 testes verdes).

### client
- [x] **T-410 · submit/stream** (REQ-410) — *Aceite:* recebe resultado/stream.
  **Status:** ✅ 2026-07-18 — `DdsClientDds`: submit (DONE + chunks via select!) e
  submit_stream (até is_final).
- [x] **T-411 · 50+ concorrentes sem deadlock** (REQ-411) — *Aceite:* stress 50+ verde.
  **Status:** ✅ 2026-07-18 — **50/50 OK em 12,6 s (4,0 tasks/s)** com UM participante
  (o deadlock de 20 do Python some por construção).

### llm-gateway
- [x] **T-420 · Worker pool (Semaphore N)** (REQ-420) — *Aceite:* N>1 em paralelo; métricas corretas.
  **Status:** ✅ 2026-07-18 — max_concurrent==N; tempo de 2 ondas; métricas assertivas.
- [x] **T-421 · Roteamento de provedor** (REQ-421) — *Aceite:* local vs cloud por constraint.
  **Status:** ✅ 2026-07-18 — trait `LlmProvider` + route por `provider_constraint`;
  mock providers provam as 3 rotas.
- [x] **T-422 · Cache + rate-limit + 429** (REQ-422) — *Aceite:* três caminhos testados.
  **Status:** ✅ 2026-07-18 — cache antes do rate limit (hit grátis); 429 retriable via
  `to_llm_error`; fix deadlock em `LlmCache::insert` (iter do DashMap durante remove).

### integração
- [x] **T-430 · E2E Rust-only + REPORT** (REQ-430, gate) — *Aceite:* E2E verde; paridade Python; REPORT.
  **Status:** ✅ 2026-07-18 — `scripts/e2e_rust_only.sh`: HTTP → orq → agente
  (DdsEngine) → llama-server → "OK" (2 chunks, latency 458 ms), agente no registry,
  loop NFCM tracejando. REPORT.md escrito.

## Gate de saída (Fase 3)
E2E Rust funciona ✓ · client ≥ 50 ✓ · NFCM integrado ✓ · gateway multi-worker ✓ · REPORT ✓
