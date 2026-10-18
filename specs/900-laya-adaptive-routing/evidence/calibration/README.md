# Calibração do checkpoint Laya — v2 (2026-10-17/18)

## Estado: wiring COMPLETO, números MELHORANDO mas AINDA INDICATIVOS

### v2 (train 840 = snapshot1050 × 80%; pin `f7451ac7…`)

| métrica | ANTES | DEPOIS |
|---|---|---|
| choice_accuracy | 0.583 | 0.583 |
| noul_accuracy | 0.799 | 0.799 |
| score_mae | 0.822 | **0.679** (−17%) |
| ece | 0.075 | 0.083 |
| brier | 0.159 | **0.158** |

8 064 records; temperature map agora CALIBRA (T₁=1,11 T₂=5,0 T₃=3,80 —
não defaults). **Noul bucket tem 672×9 = 6 048 records (>floor 2 000) —
calibrado com força estatística. Choice/score (672/1 344) ainda abaixo do
floor.**

### v1 (snapshot400; pin `a235c9b9…`) — preservada em `calib_v2/` antiga

Score_mae 0.763→0.635 (−17%). Ver `calibration_report.json` anterior.

## Dataset (ainda crescendo)

- `dataset_calib_v1.jsonl`: geração LOCAL com --resume (791→1 650+→2 000;
  ~5 ex/min na RX 7900 XTX). Rótulo por construção.
- `dataset_snapshot400.jsonl` / `dataset_snapshot1050.jsonl`: snapshots.
- Colab: **não viável por CLI > 1h** (GPU é liberada sem browser; ver
  `colab-limitacoes.md`); notebook interativo pronto em
  `tools/colab_gen/COLAB_NOTEBOOK.md` como complemento.

## Arquivos

| Arquivo | Conteúdo |
|---|---|
| `calib_v2/calibration_v1.json` | Payload de calibração (pin f7451ac7…) |
| `calib_v2/gates_v1.json` | Thresholds de abstenção + pin + counts |
| `calib_v2/train.jsonl` / `holdout.jsonl` | Split 80/20 seed 900 |
| `calib_v2/calibration_report.json` | Métricas antes/depois |
| `dataset_calib_v1.jsonl` | Dataset em crescimento (--resume) |
| `colab-limitacoes.md` | O que tentamos e por que CLI não basta |
| `tools/colab_gen/COLAB_NOTEBOOK.md` | Célula interativa p/ complementar |

## Próximos passos

1. Dataset chegando a 2 000 → recalibrar v3 (choice/score floor ~1 600/3 200 —
   ainda parcial; completo exigiria ~2 500 no train)
2. A5: se noul-bucket (o mais robusto) justificar → `--calibration-pin` no
   wf-run com pin v2 (decisões noul saem de shadow)
3. A6: sessão shadow calibrada real
