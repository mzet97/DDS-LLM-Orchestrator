# EXP-C — Contenção de Reivindicação (resultados, 2026-10-05)

**Protocolo:** dissertação §3.7.8 — descritivo, sem MEI; k∈{2,4,8} agentes com o mesmo papel (mock, slots 4), 200 tasks determinísticas sequenciais por nível, **cross-host** (agentes distribuídos .61/.64, submissão local, domínio 170). Dados: `expc/k{2,4,8}-latencies.txt`, `k{k}-per-agent.txt`, logs `/tmp/expc-k{k}-*.log` (cópias locais).

## Resultados

| k | tasks | conclusões (Σ agentes) | **execuções duplicadas** | claims perdidos (arbitragem) | p50 | p95 | max |
|---|---|---|---|---|---|---|---|
| 2 | 200 | 200 | **0** | 6 | 511 ms | 520 ms | 522 ms |
| 4 | 200 | 200 | **0** | 49 | 516 ms | 521 ms | 524 ms |
| 8 | 200 | 200 | **0** | 173 | 531 ms | 539 ms | 638 ms |

**Distribuição de carga por agente (concluídos):**
- k=2: 176/24 (.61/.64)
- k=4: 101/94/3/2 (.61×2 / .64×2)
- k=8: 50/55/43/44 (.61×2... 4 agentes .61) vs 3/2/1/2 (.64×4)

## Leitura

1. **Segurança: 0 execuções duplicadas em 600 tasks sob contenção real cross-host** — o mecanismo (claim otimista + arbitragem Exclusive + readback) cede corretamente em 100% dos conflitos observados.
2. **Conflito cresce com k, por desenho:** claims perdidos na arbitragem = 3% (k=2) → 24,5% (k=4) → **86,5% (k=8)** — a taxa de "claim perdido" é o sinal de contenção esperado (perdedores cedem; exatamente um vencedor por task).
3. **Custo de contenção é modesto:** p50 511→531 ms (+3,9% de k=2 a k=8); p95 520→539; max 522→638.
4. **Achado — skew de proximidade de rede:** em k=8, os 4 agentes da .61 (mesmo host do submissor... mesma rede/RRT menor) ficam com 96% das tasks e os 4 da .64 com 4% — a corrida de claim favorece participantes de menor latência. Implicação honesta p/ a dissertação: o balanceamento por corrida é latência-sensível; balanceamento justo exigiria política adicional (ex.: particionamento por hash de task_id → agente, já disponível via `target_agent`).
5. Sem timeouts, sem erros, sem 429 (600/600 submissões ok) — mesmo com 8 agentes e fila de claim.

## Limitações
- Detecção de duplicata por logs de agente (task_id em "task concluída"); conflitos de claim parcialmente invisíveis externamente (usado o sinal "claim perdido na arbitragem" dos logs).
- Tasks submetidas sequencialmente (sem rajada) — rajada poderia elevar a taxa de conflito; registrado como ameaça.
