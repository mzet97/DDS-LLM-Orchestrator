# Calibração do checkpoint Laya — v3 FINAL (2026-10-18)

## Estado: etapa A ENCERRADA — wiring completo, calibração aplicável, números replicam o padrão v1/v2

### v3 (train 1 280; 15 360 records; pin `a1ff7ddd…`) — recalibração final da etapa A

Entrada `calib_v3/train.jsonl` (1 600 exemplos = merge dedup do dataset 2 000);
a ferramenta re-divide 80/20 internamente (seed fixa): train 1 280 / holdout 320.

| métrica | ANTES | DEPOIS |
|---|---|---|
| choice_accuracy | 0.547 | 0.547 (idêntico — T>0 não muda argmax) |
| noul_accuracy | 0.802 | 0.802 (idem) |
| score_mae | 0.797 | **0.679 (−15%)** |
| mean_confidence | 0.781 | 0.664 (amaciada na direção da acurácia) |
| ece | 0.066 | 0.077 (piora leve) |
| brier | 0.159 | 0.161 (piora leve) |
| aurc | 0.134 | 0.136 (flat) |

Temperature map **[1.088, 5.0, 3.963]** (T₂ satura no teto 5.0 do SDK).
Abstention gates: todos **1.0** (threshold nunca dispara neste dataset —
abstenção permanece desativada na prática).

**Leitura honesta:** o score head calibra de novo (MAE −15%, replicando
v1 −17% e v2 −17% em splits distintos), mas as métricas de
ranking/confiança (ECE/Brier/AURC) ficam flat-a-pior neste holdout, e
dobrar os records (8 064→15 360) não mudou o padrão qualitativo — o limite
parece ser o checkpoint/dataset sintético, não amostragem. A calibração é
APLICÁVEL (pin verificável, `--calibration-pin` no wf-run, A6 PASS), mas os
números seguem indicativos para claims de confiança.

Nota: latência p50 3 564→2 862 ms no holdout é artefato de carga de CPU
(coleta ANTES/DEPOIS em momentos distintos), não efeito da calibração.

### v2 (train 840; 8 064 records; pin `f7451ac7…`) — usada na sessão A6

Score_mae 0.822→0.679 (−17%). Noul bucket 6 048 records (>floor 2 000);
choice/score ainda abaixo. Ver `calib_v2/`.

### v1 (snapshot400; pin `a235c9b9…`) — preservada em `calib_v2/` antiga

Score_mae 0.763→0.635 (−17%).

## Dataset (FECHADO em 2 250 bruto / 2 000 pós-dedup)

- `dataset_calib_v1.jsonl`: geração LOCAL com --resume até 2 000 (+
  250 de cauda pós-merge; ~5 ex/min na RX 7900 XTX). Rótulo por construção.
- `calib_v3/dataset_calib_v2.jsonl`: merge dedup → 2 000 → split 80/20
  (train 1 600 / holdout 400) pós-união, seed 900.
- Colab: **não viável por CLI > 1h** (GPU é liberada sem browser; ver
  `colab-limitacoes.md`); notebook interativo pronto em
  `tools/colab_gen/COLAB_NOTEBOOK.md` como complemento.

## Arquivos

| Arquivo | Conteúdo |
|---|---|
| `calib_v3/calibration_v1.json` | Payload FINAL (pin a1ff7ddd…) |
| `calib_v3/gates_v1.json` | Thresholds de abstenção + pin + counts |
| `calib_v3/train.jsonl` / `holdout.jsonl` | Split 80/20 pós-merge |
| `calib_v3/dataset_calib_v2.jsonl` | Merge dedup 2 000 |
| `calib_v3/calibration_report.json` | Métricas antes/depois completas |
| `calib_v2/…` | Idem para a v2 (pin f7451ac7…) |
| `dataset_calib_v1.jsonl` | Dataset bruto (--resume, 2 250) |
| `colab-limitacoes.md` | O que tentamos e por que CLI não basta |
| `tools/colab_gen/COLAB_NOTEBOOK.md` | Célula interativa p/ complementar |

## Pós-fechamento (fora do escopo 900)

1. Dataset real (traces de produção) no lugar do sintético — requisito para
   claims de confiança no artigo
2. Choice bucket ainda abaixo do floor (1 280 vs 2 000) — completa em
   ~2 500 train se virmos a precisar
3. Abstenção: thresholds 1.0 = nunca abstém; revisitar só com dataset real
