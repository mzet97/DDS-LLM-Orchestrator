#!/bin/bash
# =============================================================================
# T912 — CADEIA COMPLETA de treino/avaliação (portátil: SDumont, RX local, .61)
# Estágios: preflight GPU -> smoke env -> treino C0 (2B) -> treino C1 (0.8B)
#           -> dev evals -> seleção min-F1 + calibração CAL
#           -> [RUN_SEALED=1] abertura única sealed pareada + gates -> tar
#
# Uso (dentro do job ou local):  bash lncc/t912_run_all.sh
# Vars de ambiente:
#   T912_ROOT  spec 912 (default: ../ relativo a este arquivo)
#   T912_REPO  raiz do repo (default: ../../..)
#   T912_WORK  checkpoints (default: ~/.cache/tese-laya/t912_train; job: $SD_ROOT/work)
#   T912_OUT   resultados (default: $T912_ROOT/results)
#   PY         python do venv (default: python3 do PATH -> em job, venv ativado)
#   MB         minibatch (default 16; V100-32GB/H100-80GB comportam 16)
#   RUN_SEALED 1 = abre o sealed após calibração (default 1); 0 = para antes
#   SMOKE      1 = valida env com época-0 do 0.8B antes de tudo (default 1)
# =============================================================================
set -uo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
SPEC=${T912_ROOT:-$(cd "$HERE/.." && pwd)}
REPO=${T912_REPO:-$(cd "$SPEC/../.." && pwd)}
WORK=${T912_WORK:-$HOME/.cache/tese-laya/t912_train}
OUT=${T912_OUT:-$SPEC/results}
PY=${PY:-python3}
MB=${MB:-16}
RUN_SEALED=${RUN_SEALED:-1}
SMOKE=${SMOKE:-1}

export T912_ROOT="$SPEC" T912_REPO="$REPO" T912_WORK="$WORK" T912_OUT="$OUT"
mkdir -p "$WORK" "$OUT/infra"
LOG="$OUT/infra/train_chain.log"

echo "[chain] START $(date) host=$(hostname) gpu=${CUDA_VISIBLE_DEVICES:-n/a}" | tee -a "$LOG"
echo "[chain] SPEC=$SPEC WORK=$WORK OUT=$OUT PY=$PY MB=$MB RUN_SEALED=$RUN_SEALED" | tee -a "$LOG"

# ---------- preflight: GPU visível + torch funcional ----------
"$PY" - <<'EOF' 2>&1 | tee -a "$OUT/infra/preflight.log"
import torch
assert torch.cuda.is_available(), "CUDA indisponível: verifique driver/fila/torch index"
print(f"[preflight] torch={torch.__version__} dev={torch.cuda.get_device_name(0)} "
      f"cc={torch.cuda.get_device_capability(0)} bf16={torch.cuda.is_bf16_supported()} "
      f"vram={torch.cuda.get_device_properties(0).total_memory/2**30:.1f}GiB")
EOF
if [ "${PIPESTATUS[0]}" -ne 0 ]; then
    echo "[chain] ABORT: preflight CUDA falhou (driver antigo? refaça prepare com TORCH_CUDA=cu128)" | tee -a "$LOG"
    exit 1
fi

# ---------- smoke: valida modelo+dataset+save com época-0 do 0.8B ----------
if [ "$SMOKE" = "1" ] && [ ! -f "$WORK/smoke_ok" ]; then
    echo "[chain] SMOKE (C1 épocas=0) $(date)" | tee -a "$LOG"
    if T912_WORK="$WORK/smoke" "$PY" "$SPEC/training/train_t912.py" C1 --epochs 0 \
        > "$OUT/infra/smoke.log" 2>&1; then
        touch "$WORK/smoke_ok"
        echo "[chain] SMOKE OK $(date)" | tee -a "$LOG"
    else
        echo "[chain] ABORT: smoke falhou (ver $OUT/infra/smoke.log)" | tee -a "$LOG"
        exit 1
    fi
fi

# ---------- treino C0 (2B) e C1 (0.8B) ----------
for ARM in C0 C1; do
    if [ -f "$WORK/arm_$ARM/train_report.json" ]; then
        echo "[chain] $ARM ja treinado, skip" | tee -a "$LOG"
        continue
    fi
    echo "[chain] TRAIN $ARM START $(date)" | tee -a "$LOG"
    "$PY" "$SPEC/training/train_t912.py" "$ARM" --mb "$MB" --epochs 3 \
        > "$OUT/infra/train_${ARM}.log" 2>&1
    RC=$?
    echo "[chain] TRAIN $ARM END rc=$RC $(date)" | tee -a "$LOG"
    if [ $RC -ne 0 ]; then
        echo "[chain] ABORT em $ARM (rc=$RC; ver $OUT/infra/train_${ARM}.log)" | tee -a "$LOG"
        exit 1
    fi
done

# ---------- dev evals ----------
for ARM in C0 C1; do
    if [ -f "$OUT/${ARM}_dev/metrics.json" ]; then
        echo "[chain] dev eval $ARM ja pronto, skip" | tee -a "$LOG"
        continue
    fi
    echo "[chain] DEV EVAL $ARM $(date)" | tee -a "$LOG"
    "$PY" "$SPEC/evaluation/eval_t912.py" --arm-dir "$WORK/arm_$ARM" \
        --split "$SPEC/datasets/dev_t912.jsonl" --out "$OUT/${ARM}_dev" \
        > "$OUT/infra/eval_${ARM}_dev.log" 2>&1
    echo "[chain] DEV EVAL $ARM rc=$? $(date)" | tee -a "$LOG"
done

# ---------- seleção + calibração (+ sealed + gates) ----------
if [ "$RUN_SEALED" = "1" ]; then
    SEAL_ARG=""
    STAGE="SELECT+CALIB+SEALED"
else
    SEAL_ARG="--skip-sealed"
    STAGE="SELECT+CALIB (sealed ADIADO)"
fi
echo "[chain] $STAGE $(date)" | tee -a "$LOG"
"$PY" "$SPEC/evaluation/select_calibrate_seal_t912.py" $SEAL_ARG \
    > "$OUT/infra/select_calib_seal_stdout.log" 2>&1
echo "[chain] SELECT+CALIB rc=$? $(date)" | tee -a "$LOG"

# ---------- empacotamento ----------
PKG="$OUT/t912_sdumont_$(date +%Y%m%d_%H%M).tar.gz"
MODELS="arm_C0 arm_C1"
for d in "$WORK"/arm_*_cal; do
    [ -d "$d" ] && MODELS="$MODELS $(basename "$d")"
done
tar -czf "$PKG" -C "$WORK" $MODELS -C "$SPEC" results sealed
echo "[chain] PACOTE: $PKG ($(du -h "$PKG" | cut -f1))" | tee -a "$LOG"
echo "[chain] T912_RUN_ALL_DONE $(date)" | tee -a "$LOG"
