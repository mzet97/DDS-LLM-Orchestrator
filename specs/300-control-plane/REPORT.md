# Report 300 — Control Plane (`orchestrator` + `client` + `llm-gateway`)

**Data:** 2026-07-18 · **Status:** ✅ Concluída (12/12 tasks; gate da Fase 3 atingido)

---

## O que foi construído

### orchestrator (`src/dds.rs` + `src/main.rs`)
- `OrchestratorDds`: API axum → publica `Task` no tópico `Tasks` com **strength de cliente
  (10)** (T-401) — papel correto para a arbitragem (agentes=100 vencem; orq=200 fica para reaper).
- Scheduler (heap prioridade+idade, T-402), selector por especialização (T-404),
  state machine (T-406, 4 testes).
- **Reaper (T-403):** monitor do registry por staleness de heartbeat — agente morto tem
  suas tasks ASSIGNED/RUNNING reatribuídas para PENDING (retry+1, strength 200).
- **Loop de controle NFCM (T-405):** decide perfil a cada período, aplica knobs online
  (TransportPriority + OwnershipStrength via `set_qos`) e traceja `qos_decision`.
  **Limitação medida:** `latency_budget` não é mutável em runtime neste CycloneDDS
  (`dds_set_qos` → `OUT_OF_MEMORY`; repro em `spike-interop/diag-knobs`) — omitido do
  set quente (herda o valor do writer); os outros 2 knobs aplicam quentes.

### client (`src/lib.rs::dds_impl`)
- `DdsClientDds`: **UM participante servindo N tasks async** (T-410) — `submit` (DONE com
  conteúdo dos chunks via select! em 2 streams) e `submit_stream` (chunks até is_final).
- **T-411:** 50 submissões concorrentes, 1 participante → **50/50 OK em 12,6 s (4,0 tasks/s
  agregado)** — o deadlock de 20 do Python some por construção.

### llm-gateway (`src/lib.rs`)
- trait `LlmProvider` + roteamento por `provider_constraint` (LOCAL_ONLY/CLOUD_ONLY/ANY→local).
- Worker pool `Semaphore(N)` paralelo (T-420: max_concurrent==N, tempo de 2 ondas),
  cache por conteúdo **antes** do rate limit (hit não consome quota), rate limit →
  **`LLMInferenceError(429, retriable=true)`** via `GatewayError::to_llm_error`.
- Fix: deadlock em `LlmCache::insert` (iterador do DashMap segurado durante `remove`).

## Validação

| Task | Resultado |
|---|---|
| T-402/404/406 | ✅ 6 testes (ordem do heap, roteamento TEXT/VISION, state machine) |
| T-403 reaper | ✅ agente morre (SIGKILL via `mem::forget`) → task volta PENDING com retry=1 |
| T-405 NFCM | ✅ degradado → QoS_Failover; knobs aplicados; loop decide a cada período (trace) |
| T-410/411 | ✅ submit completo + stream; **50/50 concorrentes sem deadlock** |
| T-420/421/422 | ✅ pool paralelo (max==2 em 2 ondas), roteamento por constraint, cache+429 retriable |
| T-430 E2E | ✅ **Rust-only**: HTTP → orq → mesh → agente (DdsEngine) → llama-server → "OK" (2 chunks, `latency_ms=458`), agente no registry |

## Achados técnicos

1. **A API deve publicar tasks com strength de cliente (10)**, não do orquestrador (200) —
   senão a própria submissão vence a arbitragem e nenhum agente consegue clamar
   (descoberto no E2E; a guarda anti-regressão do Python sinalizava exatamente isso).
2. **`latency_budget` não muda em runtime** neste CycloneDDS (`OUT_OF_MEMORY`) — os knobs
   quentes efetivos são TransportPriority e OwnershipStrength; LatencyBudget fica no
   perfil estrutural (criação).
3. Deadlock real do `DashMap` ao segurar `iter()` durante `remove()` (em `LlmCache`).
4. A reaper robusta é por **staleness de heartbeat** (last_seen por agente), não só por
   evento de liveliness — cobre perda silenciosa e morte limpa.

## Handoff

- Binários: `orchestrator --port P --dds-domain D`, `agent --engine dds|mock`,
  `llm-gateway` (crate com provider local DDS; cloud = provider pluggável, sem chave aqui).
- Fase 4 (`400-baselines`): `qos-nfcm` já decide e é chamado pelo control loop; falta
  Zadeh/FCM/DHL atrás de `QosDecider` (T-501/502/503 parcialmente feitos por outra sessão)
  + harness 5 braços + arquivar o Python equivalente.
