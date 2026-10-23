# architecture — T904 Unified

## Fluxo da campanha

```text
[.61 — gemma-4-12b :8083]                 [local — dataset/splits]              [Colab T4 — laya 0.4.10]
generate_contrastive.py          ──scp──▶ gen.jsonl + label_a/b               (NADA antes dos dados)
  GEN (6 textos/lote, sem rótulo)          │                                     sessão laya904 (teto 20 CU)
  LABEL_A (rubrica v3, T=0)                ▼                                     ├ upload teachers 3×842MB (21MB parts)
  LABEL_B (12% estratificado, T=.2)   build_unified_dataset.py                   ├ upload train/dev/cal/distill
                                      (join+adjudicação+κ+leak audit)            ├ vm_phase1: monta professores,
                                           ▼                                     │  SHA, inferência dev+distill
                                      split_dataset.py                           ├ vm_phase2 × {A,B,C,D}: laya-train
                                      (famílias; sealed chmod 000)               │  soft-ce, 4 épocas, ckpt/época
                                           ▼                                     ├ vm_t904_eval: dev por qid/classe
                                      prepare_train_files.py                     └ calibração (split cal) pós-finalista
                                      (distill + anon uploads)                        │ download parts + SHA
                                           ▼                                          ▼
                                      apply_distillation.py ◀── teachers_preds ──┘ (gold + teacher_registry)
                                           ▼
                                      train.jsonl / train_nodistill.jsonl ──upload──▶ treinos A/B/C/D
[M8 local] run_shadow (worker+DDS+Rust, tasks_seen=0)
[M9 .61]   E2E pareado rules×T900×T904 (planner gemma4; após labels terminarem)
[M10]      laya_t904_unified local (assemble) → smoke laya.load → ZIP + SHA → decision.json
```

## Decisões estruturais

- **Fusão por pesos: NÃO** (braço F NOT_RUN); consolidação é por TREINO com replay+destilação.
- **Rótulos parciais por linha** explorados nativamente (qids sem gold/expected não treinam):
  v4_train treina SÓ qos; sintéticos treinam 12 qids; gen treina 12.
- **gold por qid** nas linhas distill, condicionado à concordância do argmax com o rótulo
  auditado (rótulo prevalece).
- Teachers montados localmente com o MESMO `assemble_checkpoint.py` do runtime-T904
  (complemento arquitetural do BASE) — carregabilidade provada antes de gastar CU.
