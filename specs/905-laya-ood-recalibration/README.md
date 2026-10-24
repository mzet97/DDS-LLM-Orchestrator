# T905 — Recalibração OOD dos gates Laya

**Estado: CONCLUÍDA (2026-10-24). Gates recalibrados 5/8 no sealed novo (450, abertura
única); candidato N2 `fa48606b…` é não-inferior ao vigente `bda679e7…` em 6/6 com
dominância — BLOCKED_WITH_TRAINED_CANDIDATE; vigente permanece; promoção é decisão do
autor. Lote de revisão humana (300 estratificados) entregue em
`laya_t900_handoff/t905/reports/human_review_batch.json`.**

## Documentos

- [PLAN.md](PLAN.md) — plano aprovado (dados medidos, gates recalibrados, protocolo)
- [gates_preregistration.json](gates_preregistration.json) — **congelados antes de qualquer treino**
- [PROMOTION-style decision](../../../laya_t900_handoff/t905/decision.json) — `BLOCKED_WITH_TRAINED_CANDIDATE` com todos os números
- [artifacts/](artifacts/) — relatórios (dev, sealed, e2e), decision, manifests, scripts (43 arquivos)
- Entrega física: `laya_t900_handoff/t905/` (pesos fora do git)

## Resultado-chave (sealed T905, n=450)

| Métrica | gate | N2 calibrado | vigente | gate |
|---|---|---|---|---|
| QoS F1 | ≥0.85 | 0.783 | 0.659 | ❌ |
| STREAM P/R/F1 | ≥0.75/0.80/0.77 | 0.939/0.821/0.876 | 0.868/0.821/0.844 | ✅✅✅ |
| Urgency F1 | ≥0.85 | **0.907** | 0.561 | ✅ |
| Planning | ≥0.95 | 0.933 | 0.738 | ❌ (gap 0.017) |
| Complexity F1 | ≥0.70 | **0.815** | 0.259 | ✅ |
| Caps F1 | ≥0.85 | 0.675 | 0.581 | ❌ (gap 0.175) |

Não-inferioridade vs vigente: **6/6** · Latência p50: 125 ms (vigente 132) ·
Shadow: `shadow_only=false` provado, tasks_seen=0 · E2E: N2 qos 0.875, latência menor.
