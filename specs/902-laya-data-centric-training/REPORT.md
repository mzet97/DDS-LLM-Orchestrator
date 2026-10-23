# REPORT T902 — Laya data-centric training

Data: 2026-10-20. Decisao: **KEEP_T900** (ver evidence/decision.json).

## Resultado cientifico principal
No sealed test inedito (528, frases e cenarios distintos, aberto 1x apos freeze), o finalista arm_B melhorou QoS Macro-F1 de 0.3233 para 0.7308 (delta +0.4074, IC95 [0.3612, 0.4594]) e urgency Macro-F1 de 0.2898 para 0.9981 (delta +0.7083, IC95 [0.6726, 0.7423]) contra o T900, com planning/complexity em 1.0. Porem o alvo primario QI_STREAM_LIKE colapsou com frases novas: recall 0.0417 (T900 0.0; gate pre-registrado 0.75). O ganho de QoS veio de CRITICAL/FAILOVER/LOW_COST/BALANCED e de prazos explicitos, nao de streaming. Sem promocao.

## Comparacao T900 x T901 x T902
| Evidencia | T900 | T901 | T902 (sealed) |
|---|---|---|---|
| Desenho | holdout reutilizado 450 | validation agrupado + golden 1x | sealed inedito 528, 1x pos-freeze |
| QoS F1 do melhor | 0.8029 (seed) | seed 0.8155 (dev agrupado) | B 0.7308 vs T900 0.3233 no mesmo sealed |
| STREAM recall | 0.2051 (golden antigo) | D 0.4872 (golden) sem significancia | B 0.0417 (frases novas) |
| Urgencia F1 | 0.6262 | candidatos piores que seed | B 0.9981 (prazos novos) |
| Conclusao | checkpoint vigente | sem melhoria, KEEP | sem promocao, KEEP_T900 |

A queda do T900 de 0.80 (golden antigo) para 0.32 (sealed) mede mudanca de distribuicao (cues explicitos novos), nao piora do modelo. STREAM so generaliza dentro das frases treinadas: dev com frases treinadas 1.0, sealed com frases novas 0.04.

## Marcos
- M0-M3: dossie SDD, politica rev.1, auditoria (STREAM antigo 81.3% sem cue), limpeza por regra (156 removidos), splits agrupados (train 2076, dev 264, cal 180, sealed 528, overlap 0).
- M4/M5: 3 bracos T4 pos-recriacao de sessao (perda registrada, retreino com persistencia por braco): A f8616426 (RLCD), B 5d7dd652 (focal), C b03a5f82 (SoftCE). Pesos A/B/C preservados localmente com SHA.
- M6: dev novo T900 QoS F1 0.3449 vs candidatos 0.92-1.0 (bootstrap exclui zero); golden antigo sem regressao (B QoS F1 0.8051 vs 0.8029).
- M7: freeze B (desempate por golden antigo), sealed unico conforme acima.
- M8: cargo workflow-engine+dds-contract verdes, Python 25 passed, shadow V04 PASS tasks=0. E2E planner NOT_RUN (servidor LLM indisponivel) registrado.
- Limitacoes: dados sinteticos com cues; sealed com distribuicao desenhada; caps no sealed fracos nos dois modelos (~0.72); shadow real com checkpoint nao executado (mock + SDK).
