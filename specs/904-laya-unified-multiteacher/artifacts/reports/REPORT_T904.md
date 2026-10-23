# REPORT_T904 — Unified Multi-Teacher Laya (2026-10-22)

Execução SDD evidence-first conforme `PROMPT_MESTRE_T904_UNIFIED_LAYA_SDD.md`.
Dossiê SDD: `rust/specs/904-laya-unified-multiteacher/`. Workspace: `laya_t900_handoff/t904/`.

## Resposta executiva

Um único estudante foi **treinado, recuperado da VM, validado e entregue**:
`models/laya_t904_unified/` (pesos `bda679e7…`, braço C = init T900 + destilação multi-teacher
+ replay + contrastivos; calibração pin `6e1543f3…`). O candidato **domina o T900 em TODAS as
métricas do sealed T904 novo** (QoS Macro-F1 0.823 vs 0.492; STREAM F1 0.919 vs 0.214;
planning 0.926 vs 0.302), com shadow DDS provado (`tasks_seen=0`, saída de shadow com o pin).
**Os gates absolutos pré-registrados NÃO foram atingidos (3 PASS / 5 FAIL)** — os alvos vinham
dos holdouts sintéticos do T900; em texto OOD real nenhum modelo os atinge (o próprio T900
faz planning 0.302 no mesmo sealed). Status: **BLOCKED_WITH_TRAINED_CANDIDATE** — candidato
preservado, T900 permanece baseline, promoção depende de decisão do autor (não afrouxamos
gate após ver resultado).

## Marcos

| Marco | Resultado |
|---|---|
| M0 | ✅ 9 pesos com SHA verificado; professores T900/T902-B/T903-C montados e `laya.load` ok local; T901 sem pesos locais (VM) → material de auditoria; contratos byte-exatos |
| M1 | ✅ 2.874 textos (política v3, evidência-antes-de-rótulo) + 414 suplementares (failover/low_cost/critical); dupla anotação 12% (κ=1.0, n=341 — **viés LLM×LLM compartilhado documentado; não é acordo humano**) |
| M2 | ✅ pool 7.022 linhas: new-gen 3.233 + t901 270 + sintéticos 2.000 + v4 real 1.449 + hardnegs 70; proveniência por linha; auditoria 8-gram vs v4_test = 0 hits |
| M3 | ✅ splits por família (108→173 famílias): train 5.842 / dev 336 / cal 265 / **sealed 567 com as 5 classes** (BAL 204, CRIT 156, STREAM 145, FAIL 32, LOW 30), congelado (`bf11db46…`), chmod 000 |
| M4/M5 | ✅ 4 braços na T4 (soft-ce, seed 904, 4 épocas): A `8a754206…`, B `b8319994…`, C `bda679e7…`, D `a83279a1…`; downloads em 39 partes com SHA VM↔local idênticos; B1/B2/B0 avaliados como baseline; **E e F NOT_RUN** (SDK sem LwF/EWC; fusão desencorajada) |
| M6 | ✅ destilação H2 confirmada com controle (C>A: QoS F1 0.863 vs 0.844; STREAM F1 0.901 vs 0.868 no dev); init T903-C prejudicou (D<C); calibração do C no split cal (pin `6e1543f3…`) |
| M7 | ✅ freeze (`decision_plan_presealed.json`) → **sealed aberto 1×** → gates 3 PASS/5 FAIL (tabela abaixo) → BLOCKED_WITH_TRAINED_CANDIDATE |
| M8 | ✅ shadow DDS real com o candidato + calibração: decisões com identidade `local:laya-t904-unified-c`/`bda679e7…`/`6e1543f3…`; **`t904-ok` aceito com `shadow_only=false`**; forjados rejeitados com causa tipada; **tasks_seen=0, tasks_written=0** |
| M9 | ✅ E2E pareado via .61 (planner gemma4 por túnel; dataset semente 32 rótulos de engenharia — caveat): T904-C > T900 (qos 0.844 vs 0.813; planning 1.000 vs 0.938); híbrido plan_validity 1.0 |
| M10 | ✅ entrega completa + smoke `laya.load` (5 perfis executados; 1 miss honesto em STREAM no smoke — recall sealed 0.90) + ZIP |

## Sealed T904 (n=567, abertura única; arm_C com calibração)

| Métrica | gate | arm_C | B0 T900 | B1 T902B | B2 T903C | gate |
|---|---|---|---|---|---|---|
| QoS Macro-F1 | ≥0.85 | **0.823** | 0.492 | 0.516 | 0.510 | ❌ |
| STREAM Precision | ≥0.75 | **0.936** | 0.304 | 0.287 | 0.355 | ✅ |
| STREAM Recall | ≥0.80 | **0.903** | 0.166 | 0.172 | 0.372 | ✅ |
| STREAM F1 | ≥0.77 | **0.919** | 0.214 | 0.215 | 0.364 | ✅ |
| Urgency Macro-F1 | ≥0.90 | 0.746 | 0.339 | 0.455 | 0.425 | ❌ |
| Planning Accuracy | ≥0.98 | 0.926 | 0.302 | 0.385 | 0.460 | ❌ |
| Complexity Macro-F1 | ≥0.94 | 0.529 | 0.293 | 0.358 | 0.324 | ❌ |
| Caps Macro-F1 médio | ≥0.97 | 0.767 | 0.655 | 0.617 | 0.667 | ❌ |
| Latência p50 (ms) | sem regressão | 148 | 151 | 151 | 151 | ✅ |

FPR BALANCED→STREAM do arm_C no sealed: ver `reports/sealed_arm_c.json` (campo `stream`).

## Registro do processo (transparência)

- Primeira varredura de eval falhou por bug de slots (caps) — corrigido antes de qualquer uso.
- O sealed foi aberto com 1 tentativa que crashou na row 0 (parser de labels) **antes de
  registrar qualquer resultado**; corrigido o parser e executada a abertura válida única.
  Nenhum dado do sealed informou seleção/ajuste.
- Arm D sofreu OOM por lançamento duplo acidental; relançado único e completou.
- Travamento de memória do PC local derrubou vigias (nenhum artefato perdido; treinos são remotos).
- κ=1.0 é concordância LLM×LLM (mesmo gerador nas duas rubricas) — tratado como consistência,
  NÃO como validade humana; nada foi rotulado de golden humano.

## Custo

Colab: 368.00 → ~353 CU na abertura do sealed (teto 20 CU respeitado; detalhe em
`budget_usage.jsonl`). Geração/rotulagem na .61 (sem custo monetário). E2E na .61.

## Próximos passos

1. **Autor**: decidir sobre promoção do candidato (domina o T900 em todas as métricas do
   sealed novo; risco: gates absolutos não batidos em urgency/planning/complexity/caps OOD).
   Se aprovar, promover com pin `bda679e7…` + calibração `6e1543f3…` e re-shadow.
2. T905 (novo teste confirmatório) exigiria novos dados e gates recalibrados para OOD real.
3. Melhorias conhecidas: urgency (prazos explícitos nos contrastivos), caps
   (sub-representados em texto real), complexity (rótulos medidos, não inferidos).
