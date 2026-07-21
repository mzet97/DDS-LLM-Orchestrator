# Plan 100 — Agente (como)

## Arquitetura (tokio, multi-task)
```
main (tokio multi-thread runtime)
├── claim_loop      : async stream de Tasks (WaitSet) → tenta claim → confirma ownership
├── executor        : para cada task assumida, chama o engine (bridge llama) e faz stream
├── heartbeat_task  : publica AgentState a cada 5s (independente da inferência)
└── shutdown        : sinal → drena writers → encerra entidades DDS
```
Um `DataSpace` (crate `dds-dataspace`) compartilhado; writers em pool.

## Módulos (crate `agent`)
```
src/main.rs         # parse args (clap): --domain --specialization --slots --llama-mode
src/claim.rs        # REQ-201/202/207: seleção + claim + confirmação de ownership
src/engine.rs       # REQ-203/206: ponte DDS ao llama-server; trait Engine { infer_stream }
src/output.rs       # REQ-204: pool de writers de TaskOutput (crossbeam MPMC)
src/heartbeat.rs    # REQ-205: AgentState periódico
```

## Mapeamento Python→Rust (paridade)
| Python | Rust | Nota |
|---|---|---|
| `task_consumer.claim_pending_task` | `claim::claim_pending` | filtro por especialização + round-robin por hash |
| `confirm_task_claim` (readback) | `claim::confirm_ownership` | poll curto pós-write |
| `dds_llm_engine` (Condition) | `engine::DdsEngine` (async) | `take_aiter` no LLM.Result |
| `_heartbeat_loop` | `heartbeat::run` | tokio interval 5s |
| MockLLMEngine | `engine::MockEngine` | para teste sem llama-server |

## Decisões técnicas
- **Engine é um trait** (`DdsEngine` real + `MockEngine`) → testes sem o C++.
- **Claim confirmation:** ownership por papel (agente=100) + readback; sob disputa, o
  perdedor cede (paridade com o Python).
- **Slots:** default 1 (serial). N slots = pool de executores (backlog; não nesta fase).
- **Timeout:** `min(deadline_restante*0.9, 120s)` (paridade).
- **Sem alocação no stream:** usar loans onde a crate permitir; medir.

## Teste (test-first)
- Unit com `MockEngine` e um `DataSpace` em memória/loopback (domínio isolado).
- REQ-208: teste que sobe 2–3 runtimes de agente + 1 publicador de 100 tasks; conta execuções
  por task (deve ser 1). Pode ser cross-process com os binários.
- Bench de propagação (criterion) vs o agente Python (REQ-209).

## Orçamento
Propagação claim→exec < 5 ms p99; CPU sob carga abaixo do Python. Medir no REPORT.
