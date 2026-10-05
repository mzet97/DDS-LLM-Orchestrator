# EXP1 — Overhead da Infraestrutura de Orquestração (resultados, 2026-10-05)

**Protocolo:** dissertação §3.7.2 + manifesto congelado. n=30 runs/condição/sistema (unidade=run, 1 workflow/run), 2 condições de atraso do backend determinístico (0 e 50 ms — o critério do EXP1 exige que a conclusão não dependa da duração simulada ✓), backend capacity 4, + **condição de controle sem orquestrador** (piso). T_extra = T_total − Σ T_backend.

## Resultados (T_extra por workflow; dados brutos em cond0/ e cond50/)

| Sistema | delay=0: médio/p50/max | delay=50: médio/p50/max |
|---|---|---|
| Controle (sem orquestrador) | 2 / 2 / 4 ms | 3 / 2 / 4 ms |
| LangGraph 1.2.11 | 59 / 59 / 62 ms | 60 / 60 / 66 ms |
| MAF 1.18.0 | 206 / 206 / 212 ms | 207 / 207 / 211 ms |
| **DDS-LLM-Orchestrator (wf-run + agent + det-responder)** | **770 / 758 / 1010 ms** | **770 / 762 / 1011 ms** |

## Leitura (pré-análise formal; TOST fica para o conjunto completo)

1. **Estabilidade entre condições ✓**: T_extra de cada sistema é constante entre delay=0 e delay=50 (ex.: MAF 206↔207 ms; DDS 770↔770 ms) — a conclusão do EXP1 não depende da duração simulada, critério explícito do experimento.
2. **Piso honesto**: controle = 2–3 ms → as diferenças entre orquestradores são atribuíveis à orquestração.
3. **H1 (DESFECHO):** |ΔT_extra| DDS↔MAF ≈ **563 ms** e DDS↔LangGraph ≈ **710 ms** — ordens de grandeza acima do MEI (±20 ms) → **fora da zona de equivalência: os orquestradores NÃO são equivalentes em overhead** nesta configuração, com DDS-LLM-Orchestrator carregando o maior T_extra.
4. **Decomposição do T_extra DDS (importante p/ a discussão):** ≈3 × 250 ms da **janela de confirmação de reivindicação** (CONFIRM_DELAY + readback RHC por estágio) ≈ 750 ms — é o custo do mecanismo de segurança anti-dupla-execução, **uma constante configurável** (trade-off segurançaxlatência), não custo do transporte DDS em si (o 850 mostrou propagação p99 0,077 ms). Recomendação de análise: reportar T_extra bruto E a decomposição (confirmação ×3 + resto).
5. **Interpretação prevista p/ a dissertação:** o dado NÃO elege vencedor — quantifica o trade-off: o orquestrador data-centric paga ~750 ms de janelas de segurança por workflow de 3 estágios (tunable), em troa de coordenação sem despachante central, descoberta e recuperação nativas (EXP4/EXP-C medem o outro lado da balança).

## Execução
- Driver: `run_exp1.sh` (idempotente; MAF/LG via harness com backend in-process; DDS via wf-run+agent(engine=dds)+det-responder; controle in-process sequencial).
- Dados: `cond0/` e `cond50/` com `maf|langgraph/run*.json` (metrics do harness), `dds/run*.json` (WF_RECORD), `controle/run*.json`, `det-delay{0,50}.jsonl`.
- p99/cauda: máximos ≈1,01 s no DDS (granularidade do confirm+poll) — reportar como observação.
