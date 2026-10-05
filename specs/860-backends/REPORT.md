# REPORT — specs/860-backends (2026-10-05)

**3/3 tasks ✅** — bloqueio EXP1/EXP3 (equivalência temporal) **destravado com medição**; protocolo congelado.

## T-860-01 — Calibração: **EQUIVALENTES**
Ferramenta: `bench.validation.calibracao` (critério congelado: |Δp50 serviço| ≤ 10 ms; razão p95 ≤ 1,2; warmup 20).
Execução ao vivo (mesma máquina, delay 50 ms, capacity 4, n=600 cada):
- HTTP stub (ThreadingHTTPServer + semáforo): serviço p50 **50,1 ms**, p95 50,2 ms; espera p50/p95 = 0.
- det-responder DDS (tokio + mpsc + attends): serviço p50 **51,2 ms**, p95 51,7 ms; espera p95 0/1,2 ms.
- Δp50 = **1,1 ms**; razão p95 = **1,030** → dentro do critério.
- Saturação extrema permanece divergente por design (429 imediato vs semáforo 30 s) — documentada (T-820-14) e fora da região operacional (c ≤ 8).
Saída: `docs/equivalencia-backends.md`; carga DDS via `wf-run` com os **prompts congelados** (o `e2e-bench` foi rejeitado pelo det-responder por falta do marcador `PROMPT_VERSION:` — comportamento correto do fixture).

## T-860-02 — Congelamento
`specs/860-backends/manifest-freeze.md`: commits Rust `853c968` + C++ `940e70a7`, SHA-256 do Qwen3.5-0.8B Q4_K_M (`bd258782…`), SHA-16 de 6 prompts + 3 workloads, parâmetros de backend congelados, protocolo de run (sanity obrigatório, TOST/Holm, MEIs; **T̃ref do H3 a congelar no piloto independente antes de EXP4**). Tag: `congelamento-pre-campanha`.

## T-860-03 — Sanity de datasets
`bench.validation.sanity` + `tests/unit/test_sanity.py` (3 testes): contagem × submissões, `trace_id` duplicado, `warmup` bool presente, spans obrigatórios não-zero em `status=ok`, vocabulário de status. Gate de aceitação de todo run da campanha.

## Pendências para 880
1. Piloto independente para `T̃ref` (H3) e revisão de n pelo poder TOST — **antes** de EXP4.
2. EXP1b: implementar `--dispatch-mode` no orquestrador (pequeno, previsto no plano).
3. Deploy distribuído (Fase 870) para EXP-C com k agentes em hosts distintos.
