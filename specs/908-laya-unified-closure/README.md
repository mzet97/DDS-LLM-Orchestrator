# T908 — Unified Closure: Fechamento de Todas as Competências >0.80

**Projeto:** DDS-LLM-Orchestrator | **Data início:** 2026-11-02
**Método:** SDD próprio, evidence-first, sem GitHub Spec Kit
**Sucessora:** T907 (comparação pareada que demonstrou C2 dominando B0 em tudo, mas 5/8 gates falharam)

## Missão

Atuar como Principal ML Engineer para corrigir as deficiências da T907 e produzir um
checkpoint T908 que atinja **F1 > 0.80 em todas as competências individualmente**, com
gates por classe e por capability, preservando os ganhos de T900–T907.

## Contrato

`QUESTIONS_V1` revision 1, 12 perguntas imutáveis:
- qos_profile (5 classes QI_*)
- needs_planning (bool)
- complexity (score 0-3)
- urgency (score 0-2)
- 8 capabilities booleanas (noul)

## Gates T908 (mais rigorosos que T907)

| Grupo | Gate |
|---|---|
| QoS | Macro-F1 >0.80; F1 das 5 classes individualmente >0.80; STREAM P e R ≥0.80 |
| Planning | Accuracy ≥0.95; F1 de ambas as classes >0.80 |
| Complexity | Macro-F1 >0.80; F1 dos 4 níveis >0.80; MAE reportado |
| Urgency | Macro-F1 >0.80; F1 dos 3 níveis >0.80; MAE reportado |
| Capacities | Macro-F1 médio >0.80; **F1 de cada uma das 8 >0.80** |
| Security | Sem regressão de recall em security_audit |
| Cobertura | ≥30 pos e ≥30 neg por capability em dev, cal e sealed |
| Evidência | Predições pareadas por exemplo, IC95%, sem vazamento |
| Promoção | Nunca automática; BLOCKED_WITH_TRAINED_CANDIDATE se qualquer gate falhar |

## Estado de partida

| Modelo | SHA | Papel |
|---|---|---|
| T904 B0 (baseline) | `bda679e7…` | Vigente, rollback |
| T905 N2 | `fa48606b…` | Experimental T905 |
| **T906 C2** | `624de57e…` | **Init preferido para T908** (domina B0 em tudo) |
| T906 C1 | `ec40027a…` | Interrompido (não usar) |

## Deficiências T907 a corrigir

| Gate | T907 C2 | Gate | Gap | Causa raiz |
|---|---|---|---|---|
| QoS F1 | 0.836 | 0.85 | 0.014 | Perto mas não atingiu |
| STREAM P | 0.947 | 0.75 | ✅ | |
| STREAM R | 0.900 | 0.80 | ✅ | |
| STREAM F1 | 0.923 | 0.77 | ✅ | |
| Urgency F1 | 0.808 | 0.85 | 0.042 | Dados urgency insuficientes |
| Planning | 0.835 | 0.95 | 0.115 | Dados planning insuficientes |
| Complexity F1 | 0.598 | 0.70 | 0.102 | Dados de complexidade não discriminam |
| Caps média | 0.795 | 0.85 | 0.055 | security_audit 0 pos; code_review baixo |
| security_audit recall | — | — | **0 pos no sealed** | Corpus não tinha exemplos |
| BALANCED→CRITICAL FP | — | 22/144 = 15.3% | Alto | Necessita contrastivos |

## Orçamento

Colab: **424,59 CU** disponíveis. Teto T908: **15 CU** (piso de parada 409,5).

## Fases

M0 → M1 (policy v4 + novos dados) → M2 (dataset) → M3 (splits) → M4 (treino) → M5 (eval) → M6 (calibração) → M7 (freeze) → M8 (sealed 1×) → M9 (Rust/DDS) → M10 (entrega)
