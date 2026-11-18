#!/bin/bash
# =============================================================================
# T912 no SDumont (LNCC) — PREPARAÇÃO (rodar UMA vez NO LOGIN NODE, com internet)
# Projeto: ver alocação do orientador (proposta SD n. 249768, UERJ)
#
# Faz: clone do repo -> venv -> pip (torch CUDA + requirements provados)
#      -> download dos modelos Qwen3.5 (0.8B/2B) no cache HF do scratch
# Tudo em $SD_ROOT (default $SCRATCH/t912): nós de computação NÃO leem /prj.
#
# Uso:  bash sdumont_prepare.sh
# Vars: SD_ROOT (default $SCRATCH/t912)
#       TORCH_CUDA (default cu126; use cu128 p/ H100 se driver reclamar,
#                   ou refaça o venv com torch mais antigo se driver < 525)
# =============================================================================
set -euo pipefail

module load anaconda3/2024.02_sequana

SD_ROOT=${SD_ROOT:-$SCRATCH/t912}
TORCH_CUDA=${TORCH_CUDA:-cu126}
REPO_URL=https://github.com/mzet97/DDS-LLM-Orchestrator.git
BRANCH=t912-dose-capabilities
SPEC=src/rust/specs/912-dose-capabilities

mkdir -p "$SD_ROOT"
echo "[prepare] SD_ROOT=$SD_ROOT  TORCH_CUDA=$TORCH_CUDA"

# ---------- 1. código ----------
if [ ! -d "$SD_ROOT/repo/.git" ]; then
    git clone -b "$BRANCH" "$REPO_URL" "$SD_ROOT/repo"
else
    (cd "$SD_ROOT/repo" \
        && git fetch origin \
        && git checkout "$BRANCH" \
        && git reset --hard "origin/$BRANCH")
fi

# ---------- 2. ambiente ----------
if [ ! -d "$SD_ROOT/venv" ]; then
    python -m venv "$SD_ROOT/venv"
fi
PIP="$SD_ROOT/venv/bin/pip"
"$PIP" install -U pip wheel packaging

# torch primeiro (index CUDA dedicado; sem pandas/numpy conflitos)
"$PIP" install "torch==2.9.1" --index-url "https://download.pytorch.org/whl/$TORCH_CUDA"

# pacotes provados na RX local (mesmas versões: comparabilidade entre plataformas)
"$PIP" install -r "$SD_ROOT/repo/$SPEC/lncc/requirements-t912.txt"

# xformers opcional (acelera attention; versão casada com torch 2.9.1)
"$PIP" install "xformers==0.0.35" --index-url "https://download.pytorch.org/whl/$TORCH_CUDA" || \
    echo "[prepare] xformers falhou (prossegue sem; não é obrigatório)"

# ---------- 3. modelos HF (download agora; nós de computação ficam OFFLINE) ----------
export HF_HOME="$SD_ROOT/hf-cache"
HF_PY="$SD_ROOT/venv/bin/python"
"$HF_PY" - <<'EOF'
from huggingface_hub import snapshot_download
for m in ("unsloth/Qwen3.5-2B", "unsloth/Qwen3.5-0.8B"):
    p = snapshot_download(m)
    print("[prepare] OK", m, "->", p)
EOF

# ---------- 4. pronto ----------
cat <<EOF

[prepare] CONCLUÍDO. Próximo passo (submeter o job):

  cd $SD_ROOT/repo/$SPEC
  sbatch lncc/sdumont_train.sbatch

  monitorar:   squeue -u \$USER
  acompanhar:  tail -f \$SD_ROOT/repo/$SPEC/results/infra/train_chain.log
EOF
