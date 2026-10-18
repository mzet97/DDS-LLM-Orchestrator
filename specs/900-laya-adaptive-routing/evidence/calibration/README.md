# Calibração do checkpoint Laya (T-900-A) — 2026-10-17

## O que foi feito

1. **Dataset sintético por construção** (`gen_dataset.py`): spec amostrado
   (idioma/qos/planning/caps/urgência/complexidade) → gemma-4-12b ESCREVE o
   pedido que exige o spec → rótulo é verdade por construção (ruído do
   gerador documentado; filtros de idioma/dedup/tamanho). Formato JSONL
   NATIVO do SDK (`evals.Dataset.from_jsonl`), tags como lista, alvo
   numérico (`targets.py`: one-hot por critério/[não,sim]/níveis — a ordem
   dos marcadores é a ordem declarada da pergunta).
   - Snapshot congelado p/ calibração: `dataset_snapshot400.jsonl` (400).
   - Geração COMPLETA (2000) segue em fundo no `dataset_calib_v1.jsonl`
     (resumível; ~5 ex/min → ~5,5 h; recalcibrar ao completar).
2. **Calibração** (`calibrate_laya.py`, SDK nativo): split 80/20 seed 900 →
   `records_from_labeled` (logits CPU) → `fit_temperature_map` (T por tipo)
   → `fit_abstention_thresholds` + `fit_binning_map` → `save_calibration`.
   - `calibration_v1.json` pin sha256 **a235c9b90a2624ba…** (`gates_v1.json`).
3. **Wiring**: `RealLayaAdapter(calibration_path=…)` carrega a calibração e
   publica o pin em `calibration_revision`; `DecisionPolicy.calibration_pin`
   no validator Rust — `shadow_only=false` SOMENTE quando a revisão casa com
   o pin (teste `calibration_pin_governs_shadow_mode`).

## Resultados no holdout (80) — HONESTOS

| métrica | ANTES | DEPOIS |
|---|---|---|
| choice_accuracy | 0.525 | 0.525 |
| noul_accuracy | 0.806 | 0.806 |
| score_mae | 0.763 | **0.635** (−17%) |
| ece | 0.068 | 0.073 |
| brier | 0.164 | 0.164 |

Leitura sem viés: a calibração MELHOROU o MAE de score, manteve acurácias e
**piorou levemente o ECE** — com 400 exemplos os floors do SDK
(MIN_BUCKET_N=2000/bucket) não são atingidos: o mapa de temperaturas fica
nos defaults para quase todos os buckets. **Conclusão honesta: esta
calibração é INDICATIVA, não estatisticamente defensável; o gate de
confiança NÃO deve ser congelado com estes números.** O wiring
(checkpoint→pin→validator) está pronto e é o deliverável; números
defensáveis exigem o dataset completo (rodando) ou maior.

## Ameaças à validade

- Dataset sintético gerado pelo MESMO gemma que o planner (viés de
  distribuição); rótulo por construção mitiga mas não elimina ruído.
- choice_accuracy 0.525 no holdout reflete o desbalanceamento do argmax
  QoS (o laya tende ao BALANCED; o spec sorteia perfis uniformemente).
- Holdout pequeno (80): intervalos largos.
