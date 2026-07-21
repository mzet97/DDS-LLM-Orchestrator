# Plan 300 — Control plane (como)

## orchestrator (bin)
```
src/main.rs        # tokio + axum; parse args (clap): --port --dds-domain --qos-manager
src/api.rs         # REQ-401: rotas axum -> enfileira Task
src/scheduler.rs   # REQ-402: BinaryHeap por (prioridade, created_at)
src/registry.rs    # REQ-403: consome AgentState stream; marca mortos (liveliness)
src/dispatch.rs    # REQ-404: selector por especialização/capacidade
src/control.rs     # REQ-405: loop async; usa qos_nfcm::{Nfcm, StabilityController}
src/state.rs       # REQ-406: máquina de estados de Task
```
- **QoS (REQ-405):** `FuzzyMetrics` (orch-common) → `Nfcm::infer` → `StabilityController::update`
  → `DataSpace::set_online_knobs(profile)` (só mutáveis). Trace via `tracing` (logger `qos_decision`).
- Reusar `qos-nfcm` (pronto) e `dds-dataspace` (Fase 2).

## client
```
src/lib.rs         # Client { submit, submit_stream }; UM DataSpace, N tasks async
```
- **REQ-411:** um participante compartilhado; inflight via `Semaphore`; cada submit é uma
  Future. Teste sobe 50+ submits concorrentes num único cliente.

## llm-gateway
```
src/lib.rs         # Gateway { route, infer }
src/workers.rs     # REQ-420: pool com Semaphore(N)
src/providers.rs   # REQ-421: LocalLlamaCpp | CloudOpenRouter
src/cache.rs       # REQ-422: cache (moka) + rate-limit; 429 no drop
```

## Paridade (Python → Rust)
| Python | Rust |
|---|---|
| `server.py` (aiohttp) | `orchestrator::api` (axum) |
| `scheduler.py` | `orchestrator::scheduler` |
| `registry.py` | `orchestrator::registry` |
| `selector.py`/`dispatcher.py` | `orchestrator::dispatch` |
| `main._nfcm_qos_check` | `orchestrator::control` (usa `qos-nfcm`) |
| `client/dds_client.py` | `client` |
| `llm_gateway/main.py` | `llm-gateway` |

## Teste
- E2E (REQ-430): binários reais em domínio isolado + MockEngine no agente.
- REQ-411: stress de 50+ clientes concorrentes (sem deadlock; comparar com o teto de 20 do Python).
- REQ-405: teste de decisão (degradado→Failover) + trace.

## Orçamento
E2E funcional; cliente ≥ 50 concorrentes; loop de controle sem regressão de latência.
