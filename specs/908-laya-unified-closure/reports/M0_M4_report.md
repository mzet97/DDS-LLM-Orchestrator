# T908 — Relatório M0 (auditoria) + estado M2a

**Data:** 2026-11-03

## M0 — Atlas de erros do sealed T907 (VERIFICADO)

Fonte: predições pareadas por exemplo (`t907/reports/sealed_t907_{c2,b0,n2}_predictions.jsonl`,
n=351, split `df8ae655…`), processadas por `t908/scripts/m0_error_atlas.py` →
`t908/reports/m0_error_atlas.json`.

### Erros dominantes do C2 (candidato vigente, 624de57e…)

| Padrão | Contagem | Diagnóstico |
|---|---|---|
| QoS CRITICAL→BALANCED | 22/73 (30% do gold CRITICAL) | fronteira consequência-de-produção mal expressa no treino |
| Urgency 1→0 | 48/188 (25,5%) | subestima prazo intermediário; level 1 recall 0.675 |
| Planning missed_plan | 53 vs 5 false_plan | under-prediction sistemático de needs_planning |
| Complexity nível 3 | **0 suporte no sealed** | suplemento T907 só gerou k∈{1,2,3} ⇒ cx∈{0,1}; nível 3 inavaliável |
| Complexity nível 0 | F1 0.651 | confunde com nível 1 |
| security_audit | F1 0.524 (19 FN, 1 FP) | C2 REGREDIU vs B0 (0.761): perde pedidos de auditoria |
| summarization | F1 0.712 (8 FP, 9 FN) | fronteira com translation/code_review |
| python_refactor | F1 0.812 (11 FN) | ainda abaixo do gate |
| code_review | F1 0.816 (12 FN) | abaixo do gate 0.80 fechamento? (gate novo = 0.80 por cap) |
| translation | F1 0.807 (7 FN) | no limite |

### Pareado C2 vs B0 (demonstra que continuação vale; retratação da regressão como dominante)

- C2 erra/B0 acerta: qos 17, urg 46, plan 24, caps 44, cx 23
- B0 erra/C2 acerta: qos 66, urg 94, plan 49, caps 245, cx 192
- Balanço: C2 DOMINA em todas as dimensões; as regressões pontuais (security_audit, urgência)
  são exceções localizadas que o T908 ataca com dados direcionados.

## Causa raiz do corpus (corrigida no M2a)

1. `gen_supplement.py` T907: `k ∈ {1,2,3}` ⇒ complexity só 0/1, **nível 3 nunca gerado**
2. `urgency_gold=1` fixo no suplemento ⇒ escada 0/1/2 desbalanceada
3. QoS do suplemento sem contrastes CRITICAL×BALANCED no mesmo domínio
4. security_audit: 30 pos no sealed mas treino com frases template (o gerador omitia phrasings implícitos)

## M2a — suplemento direcionado (em execução na .61)

`gen_t908_supplement.py --batches 110 --seed 908` (gemma-4-12b, thinking ON, maxtok 7000):

- S1_qos_contrast: CRITICAL (consequência explícita de produção/SLA) × BALANCED (sem consequência) no MESMO domínio
- S2_urgency: escada 0 (sem prazo) / 1 (prazo real intermediário) / 2 (produção parada)
- S3_complexity: k ∈ {1,5,7,8} ⇒ cx 0/2/3 (nível 3 agora gerado)
- S4_caps: security_audit com phrasings implícitos ("validar que não pode ser explorado"), summarization contrastivo

Gold por CONSTRUÇÃO (k_stages→cx/planning; banco de prazos→urgency; cenário→caps/qos) com
filtro de concordância LLM nas dimensões constrangidas (`label_t908_supplement.py` + 
`build_t908_pool.py` fazem o drop de divergentes). Polo de treino final =
pool T907 (5.237) + suplemento filtrado (~300).

## M4 — gates congelados

`specs/908-laya-unified-closure/manifests/gates_preregistration.json` (congelado 2026-11-03,
ANTES de qualquer treino): fechamento 0.80 em TODAS as competências (QoS macro e por classe,
STREAM P/R/F1, planning, complexity por nível c/ suporte mínimo 30, urgency por nível, 8 caps
por capability), sem regressão de security_audit abaixo de 0.76, guardrail de recall BALANCED
≥0.80, dominância vs C2, orçamento máx 15 CU (piso 409.5), promoção nunca automática.

## Orçamento

Colab: 424,59 CU no congelamento. Teto T908: 15 CU. Geração/rotulação na .61 = 0 CU (local).
