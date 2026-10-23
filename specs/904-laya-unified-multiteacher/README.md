# specs/904-laya-unified-multiteacher — T904 Unified

**Objetivo:** treinar UM estudante Laya (`laya_t904_unified`) que consolide os conhecimentos
de T900 (planning/complexity/caps), T902-B (urgency/QoS) e T903-C (STREAM) com dados
auditados (política v3), replay multitarefa, destilação seletiva e contrastivos
BALANCED×STREAM — validado no worker Python + CycloneDDS + validator Rust.

**Estado:** M0 concluído; M1 geração em andamento na VM .61; M2/M3 ferramentas prontas.

**Decisões do autor (2026-10-21):** gerador na VM .61 (gemma-4-12b :8083); teto **20 CU**
Colab; E2E via .61. Se os gates falharem: candidato preservado como NÃO PROMOVIDO, T900
permanece baseline.

## Documentos

`spec.md` (REQ-904U-*) · `contracts_audit.md` · `checkpoint_inventory.json` ·
`labeling_policy_v3.md` · `teacher_policy.md` · `dataset_card.md` · `split_protocol.md` ·
`experiment_plan.md` · `budget.md` · `risks.md` · `tasks.md` · `traceability.md` ·
`REPORT.md`/`decision.json` (no fim) · `artifacts/` · `ADRs/`.

## Pipeline (scripts em `laya_t900_handoff/t904/scripts/`)

```text
generate_contrastive.py  (.61: gen 6-textos/lote + label A + label B 12%)
build_hardnegs.py        (local: hard negatives determinísticos)
build_unified_dataset.py (join + adjudicação + κ + proveniência + anti-vazamento)
split_dataset.py         (splits por família + sealed segregado chmod 000)
prepare_train_files.py   (distill.jsonl + uploads)
drive_colab.py           (Colab: sessão/teto/upload-parts/teachers/train/fetch)
apply_distillation.py    (gold por qid vencedor no dev + teacher_registry)
vm_t904_*.py             (VM: env/professores/treino/avaliação)
verify_checkpoint.py     (smoke local laya.load + 1 exemplo por perfil)
```
