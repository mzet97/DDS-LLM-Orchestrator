#!/bin/bash
# =============================================================================
# T912 — job SLURM SDumont (LNCC): treino Qwen3.5 2B/0.8B + avaliação completa
# Submeter DE DENTRO DE: src/rust/specs/912-dose-capabilities/
#   sbatch lncc/sdumont_train.sbatch
#
# FILA: default sequana_gpu_prod (nós 4x V100-32GB). Alternativas:
#   - sequana_gpu_dev  : teste rapido (~20 min), use p/ validar a fila
#   - hpc3_gpu_prod    : nós 4x H100-80GB (geração nova; se o projeto tiver acesso)
#   - confira `sinfo -s` e edite PARTITION abaixo conforme a alocação do projeto
#
# Estimativa (V100, mb16): C0 ~1-2h + C1 ~0,5-1h + evals ~2-3h => cabe em 8h.
# UA: nos de GPU contam UA diferenciada; confira a cota do projeto 249768.
# =============================================================================
#SBATCH --nodes=1
#SBATCH -p sequana_gpu_prod
#SBATCH -J t912
#SBATCH --gpus=1
#SBATCH --cpus-per-task=8
#SBATCH --time=08:00:00
#SBATCH --output=slurm-t912-%j.out
#SBATCH --error=slurm-t912-%j.err

set -uo pipefail

module load anaconda3/2024.02_sequana

SD_ROOT=${SD_ROOT:-$SCRATCH/t912}
SPEC=$SLURM_SUBMIT_DIR                     # .../src/rust/specs/912-dose-capabilities

export PY="$SD_ROOT/venv/bin/python"
export T912_WORK="$SD_ROOT/work"
export HF_HOME="$SD_ROOT/hf-cache"
# nós de computação NÃO têm internet: cache HF do prepare, modo offline
export HF_HUB_OFFLINE=1
export TRANSFORMERS_OFFLINE=1
export TOKENIZERS_PARALLELISM=false
export RUN_SEALED=${RUN_SEALED:-1}

echo "[sbatch] node=$(hostname) job=$SLURM_JOB_ID gpu=$(nvidia-smi -L 2>/dev/null | head -1)"
nvidia-smi || true

bash "$SPEC/lncc/t912_run_all.sh"
RC=$?

echo "[sbatch] t912_run_all rc=$RC"
echo "[sbatch] resultados em: $SPEC/results/ (tar t912_sdumont_*.tar.gz = modelos+resultados)"
exit $RC
