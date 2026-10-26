# T906 — Human-Grounded Capability Recovery

**Estado: M0 em execução (2026-10-25).** Sucessora de T904/T905 conforme
`PROMPT_MESTRE_T906_LAYA_HUMAN_GROUNDED_SDD.md`. Objetivo: checkpoint único T906
corrigindo caps rust/python (positivos), recall BALANCED, preservação de predições
por exemplo e naming correto dos níveis de validação.

## Decisões do autor (implícitas na aprovação do plano)

- Sem anotadores humanos agora → filas cegas A/B PRONTAS, rótulos SILVER/medidos,
  `human-grounded` = PENDING (HUMAN_REVIEW_PENDING.md)
- Orçamento Colab: teto **14 CU** (piso de parada ~336,5 CU)
- Corpus novo na .61 (gemma4, pipeline no-think)
- 3 braços: A (init B0) / B (init N2) / C (N2 + oversampling caps)
- E2E: tentar Nível 4 via .61; fallback Nível 3 nomeado como decision_planning_benchmark

## Documentos

spec · labeling_policy_v4 · annotation_guidelines · data_card · experiment_plan ·
cost_budget · contracts · hypotheses · test_plan · tasks · traceability · risks ·
manifests/ (baseline_inventory, gates_preregistration, splits, lineage, decision_plan) ·
adrs/001-004 · artifacts/ · REPORT_T906.md · DECISION.json · REPRODUCE.md
