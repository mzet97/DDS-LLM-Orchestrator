# Campanha M6 COMPLETA — 4 braços × N=30 (2026-10-17, execução real)

Modelo de decisão: laya==0.4.10 (ckpt 891102d3…, CPU). Planner:
gemma-4-12b-it-UD-Q4_K_XL (sha256 90fd944d…, prebuilt ROCm b11671, -np 1,
~60 tok/s, HTTP direto — hop DDS do gateway fora da latência [ameaça
declarada]). Validação de planos: **bin Rust `dag_validate`** (mesmas regras
do engine M5). 120 amostras por braço (30 runs × 4 rows: 2 simples + 2
planning). Manifests por run nestes diretórios.

## Resultados

### Qualidade (1 pass, dataset completo 32 PT/EN)
| Braço | qos acc | planning acc | planos válidos (Rust) | nós médios | invocações planner |
|---|---|---|---|---|---|
| static/rules | **0.9062** | **0.9375** | — | — | 0 |
| laya | 0.7500 | 0.8125 | — | — | 0 |
| planner_only | — | — | 0.7188 | 2.57 | 23/32 |
| laya+planner | 0.7500 | 0.8125 | **1.0000** | 2.56 | **9/32** |

### Latência/custo (N=30 × 4 rows)
| Braço | P50 | P95 | tokens planner (total/média por chamada) | outliers MAD>3.5 |
|---|---|---|---|---|
| static/rules | **0 ms** | **0 ms** | 0 | 0 |
| laya | 1.560 ms | 1.949 ms | 0 | 2 |
| planner_only | 35.156 ms | 42.492 ms | 102.840 / 857 | 0 |
| laya+planner | **22.441 ms** | 42.417 ms | 92.325 / 1.026 | 0 |

## Leitura honesta (sem viés para nenhum braço)

1. **Regras vencem em qualidade no seed** (0.906 vs 0.750) e custam zero —
   esperado: o dataset semente é construído por keywords PT/EN. NÃO é
   alegação geral sobre o Laya.
2. **O gate do Laya paga o planner só onde ele rende**: laya+planner tem
   validade de planos 1.00 (vs 0.72 do planner incondicional) porque invoca
   o planner em apenas 9/32 — nos pedidos genuinamente multi-etapa, onde o
   gemma decompõe bem; no resto segue caminho direto.
3. **O custo do planner domina**: ~30 s por 4 decisões com planejamento
   incondicional; com o gate do Laya o P50 cai 36% (22,4 s) e o total de
   tokens também cai (92k vs 103k), apesar de planos médios MAIORES
   (1.026 vs 857 tokens — só os casos complexos sobreviveram ao gate).
4. Decisão Laya em CPU: P50 1,56 s (12 perguntas por chamada) — o custo
   fixo por requisição no braço híbrido.

## Ameaças à validade (além das congeladas no PROTOCOL_M6)

- Dataset seed rotulado por engenharia (não calibration-grade).
- Confiança do laya NÃO calibrada (aviso do próprio checkpoint).
- Planner medido via HTTP direto; o hop DDS do llm-gateway não entra.
- Decisão em CPU / planner em GPU (assimetria declarada no manifest).
- Subset de latência fixo (4 rows): comparativo entre braços válido; não
  extrapolar para workload arbitrário.
