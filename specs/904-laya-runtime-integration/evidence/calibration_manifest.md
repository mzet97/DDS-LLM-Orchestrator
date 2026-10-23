# Manifesto de calibração — T904/M4 (2026-10-21)

## Estado: **SHADOW_ONLY para o T900** (gate M4 satisfeito na forma "SHADOW_ONLY explícito")

### 1. Qual calibração existe e a qual checkpoint pertence

| Arquivo | Pin (sha256) | `model_id_or_path` no payload | Checkpoint vinculado |
|---|---|---|---|
| `specs/900-laya-adaptive-routing/evidence/calibration/calibration_v1.json` | `a235c9b9…` | `convaiinnovations/laya` | BASE `891102d3…` |
| `…/calib_v2/…` (v2) | `f7451ac7…` | `convaiinnovations/laya` | BASE |
| `…/calib_v3/…` (v3) | `a1ff7ddd…` | `convaiinnovations/laya` | BASE |
| `…/calib_v4/calibration_v4.json` (SPROUT real) | `10ef5945…` (gates_v4) | `convaiinnovations/laya` | BASE |

**Nenhuma calibração existente é vinculada ao T900 `35813054…`.** A aplicação da calibração do
BASE ao T900 é PROIBIDA (PROMPT T904 §M4.0) — temperaturas ajustadas aos logits de outro
modelo não são transferíveis. Portanto, no runtime T904 o adapter opera SEM calibração:
`calibration_revision=""` ⇒ o validator Rust (`decision.rs`) classifica toda decisão aceita
como `shadow_only=true` e NADA é materializado.

### 2. Confirmação empírica desta execução (M3 smoke)

O SDK emite para o T900 o MESMO aviso de temperaturas inválidas que emitia para o BASE
(`evidence/m3-smoke-real-t900.json`): "Treat confidence from the affected entries as
uncalibrated". A calibração por temperatura NÃO corrige argmax (README v4: choice_accuracy
idêntico antes/depois) — não há atalho de calibração para o problema de classificação.

### 3. Opção registrada — recalibração do T900 (NÃO EXECUTADA)

- Ferramenta: `workers/laya-worker/tools/calibrate_laya.py <dataset> <outdir> --limit N --seed 900`.
- Split permitido e independente: derivado de `CARROT-LLM-Routing/SPROUT` via
  `tools/colab_calib/main.py` (seed 900) — os MESMOS critérios do v4 (rótulo por medição
  Pareto). NÃO usar os conjuntos selados T902 (`db457693…`) nem T903 (`7578942a…`).
- Custo estimado LOCAL (CPU-only; torch 2.14.1+cpu; p50 ≈ 2,9–3,6 s/decisão):
  - 1 000 exemplos ≈ 50–60 min
  - 12 000 (v4 train) ≈ 10–12 h
  - GPU (Colab L4, 8,6 ex/s na coleta): 12k ≈ 25–35 min — exige autorização de custo.
- Cuidado de desenho: para comparação justa BASE×T900, a recalibração deve usar split NÃO
  usado como holdout de avaliação de nenhum dos dois (v4_test foi holdout do BASE).
  Recomendação: novo split temporal/por seed do SPROUT train, registrado aqui antes da execução.

### 4. Consequência operacional

- M5 (shadow) NÃO depende de calibração — pode e deve rodar com `calibration_revision=""`.
- M6 (E2E ativo) permanece **BLOQUEADO por design** enquanto não houver calibração aceita do
  T900 (`DecisionPolicy.calibration_pin` casando) — REQ-904-007; registrar NOT_RUN/BLOCKED.
