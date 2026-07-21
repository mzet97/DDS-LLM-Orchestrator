# Report 100 — Agente Rust (`agent`)

**Data:** 2026-07-18 · **Status:** ✅ Concluída (8/8 tasks)
**Substitui:** `src/orchestrator/agent/` (~2,0k LOC Python)

---

## O que foi construído

| Componente | Conteúdo |
|---|---|
| `src/engine.rs` | trait `Engine` + `MockEngine` (T-201) |
| `src/engine_dds.rs` | `DdsEngine` — ponte ao llama-server via `LLM.Inference*` com correlação por `request_id`, timeout por deadline da task, erro via `LLM.InferenceError` (T-205) |
| `src/claim.rs` | elegibilidade (especialização/target_agent), claim, confirmação de ownership |
| `src/dds.rs` | `AgentDds`: claim loop sobre `stream_tasks`, confirmação via **`read_task_mesh` (estado arbitrado do RHC)**, processamento em tasks tokio com slots, publicação de chunks pelo pool de writers (T-202/203/204) |
| `src/heartbeat.rs` | `AgentStatus` com atômicos + EMA + uptime real; heartbeat dedicado a cada 5 s (T-206) |
| `src/main.rs` | binário `--engine dds|mock`, tracing com default `info` |

## Validação (evidência de execução)

| Task | Resultado |
|---|---|
| T-201 Engine/MockEngine | ✅ 3 testes verdes (chunks previsíveis, delay, trait Send+Sync) |
| T-202/203/204/206 E2E | ✅ `tests/agent_e2e.rs`: 10/10 tasks claim→RUNNING→DONE com atribuição correta, 30/30 chunks publicados pelo pool, heartbeat no AgentRegistry (`completed_total=10`, `uptime>0`) |
| T-205 DdsEngine real | ✅ llama-server (Phi-4-mini): round-trip real — request → chunks correlacionados ("Hello", 2 chunks) |
| T-207 A/B coexistência | ✅ 1 Rust + 1 Python, 100 tasks, **0 execução dupla**; arbitragem de ownership consistente (winner da rodada: rust, menor GUID) |
| T-208 bench | Throughput do claim loop: **4,02 tasks/s** (100 tasks em 24,9 s; limitado pelo `CONFIRM_DELAY=250 ms` sequencial — mesmo patamar do Python ~4 claims/s) |

## Achados técnicos centrais (documentados para a tese)

1. **Arbitragem de Exclusive Ownership em empate de strength:** o CycloneDDS decide por
   **menor GUID** (determinístico, mesh-wide — `dds_rhc_default.c:1023-1066`). Em igualdade
   de strength (100=100), **um lado vence todas as disputas da rodada**.
2. **O write-through no cache quebra a confirmação de claim** (o 2º a clamar sempre se
   auto-confirma pelo echo local). Removido do `DataSpace` real: o readback usa
   **`read_task_mesh`**, que lê o estado **arbitrado pelo RHC** — consistente nos dois lados.
   O stub Python teve o mesmo tratamento (leitura pela RHC, não pelo cache com write-through).
3. **Semântica do cache igualada ao Python** (`_tasks_cache`): rejeita regressão
   (status para trás / assigned_agent preenchido→vazio; `retry_count` maior sempre vence);
   fora isso **last-write-wins por chegada** — nunca "maior timestamp".
4. **Resposta do llama-server pode ser vazia** (decisão do modelo) — testes devem afirmar
   o round-trip do protocolo, não o conteúdo.

## Handoff (para Fase 3)

- `agent` binário pronto para o control plane: `--engine dds` (produção) ou `--engine mock` (teste).
- Throughput do claim loop é limitado pelo confirm sequencial de 250 ms — se a Fase 3 exigir
  mais, paralelizar as confirmações (janela deslizante) é a alavanca (mantendo o readback pela RHC).
- Lembretes operacionais: `CYCLONEDDS_STATIC=1`, domínios isolados por teste,
  llama-server via `/home/mzet/.cache/llama-build/bin/llama-server --enable-dds`.
