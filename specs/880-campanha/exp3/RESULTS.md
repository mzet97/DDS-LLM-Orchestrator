# EXP3 — Escalabilidade e Concorrência (resultados, 2026-10-05)

**Protocolo:** dissertação §3.7.6 + manifesto congelado. Série principal determinística, single-host, domínio 188: orquestrador + 4 agentes mock (4 slots cada = 16 slots). Blocos de c workflows concorrentes (c∈{1,2,4,8}), 30 blocos/nível (n = c×30 workflows), latência medida no cliente por workflow (soma dos estágios). Dados: `c{1,2,4,8}-latencies.txt`.

## Resultados (450 workflows, 0 falhas)

| c | workflows | p50 (ms) | p95 (ms) | média (ms) | vazão (wf/s) | E(c) |
|---|---|---|---|---|---|---|
| 1 | 30 | 1529 | 1532 | 1528 | 0,6 | 1,00 |
| 2 | 60 | 1530 | 1545 | 1531 | 1,2 | 1,00 |
| 4 | 120 | 1535 | 1632 | 1541 | 2,3 | 0,96 |
| 8 | 240 | 1562 | 1681 | 1575 | 4,1 | 0,85 |

E(c) = Throughput(c)/(c·Throughput(1)).

## Leitura (pré-TOST)

1. **Latência estável até c=4** (p50 1529→1535 ms, +0,4%) e **degradação suave em c=8** (p50 +2,1%; p95 +9,7%) — sem cliff, sem colapso de cauda.
2. **Vazão escala ~linear até c=4** (0,6→2,3 wf/s) e **sublinear em c=8** (4,1 wf/s, E(8)=0,85) — limites: 3 × janela de confirmação de 250 ms por workflow (T_extra dominante do EXP1) + serialização do RHC.
3. **Sem crashes, sem perda, sem duplicação** em 450 workflows com 16 slots e 4 agentes — o cenário que crashava em julho (">2 concorrentes") está encerrado também sob carga concorrente real (consistente com T-850-05).
4. Contexto p/ a dissertação: com LLM real (EXP2, ~5,3 s/workflow), o T_extra de 770 ms seria ~14,5% — a curva de E(c) com inference-bound tende a ficar ainda mais plana.

## Pendências do pacote EXP3
- Série complementar com LLM real (se o backend distinguir saturação — §3.7.6).
- Análise TOST formal (razão de vazão c=8 vs MEI ±15%: razão observada E(8)=0,85 → fora por pouco; formal no conjunto).
