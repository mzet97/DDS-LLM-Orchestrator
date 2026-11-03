#!/usr/bin/env bash
# T-900-A1 — setup da VM Colab p/ geração do dataset de calibração.
# Fonte única do modelo: gemma-4-12b-it-UD-Q4_K_XL (sha256 90fd944d… — B-BENCH).
set -euo pipefail
cd "$(dirname "$0")"

GGUF=models/gemma-4-12b-it-UD-Q4_K_XL.gguf
EXPECTED_SHA="90fd944d227e9d9b68e7e2c7d5b57b79d4c66ed521b0919fbbd932cf834f6f8e"

# 1) modelo (baixa 1x por VM; mantém no disco local da sessão)
if [ ! -f "$GGUF" ]; then
  mkdir -p models
  echo "[setup] baixando gemma-4-12b-it-UD-Q4_K_XL.gguf (~7,4 GB)…"
  curl -L --retry 3 --progress-bar -o "$GGUF.part" \
    "https://huggingface.co/unsloth/gemma-4-12b-it-GGUF/resolve/main/gemma-4-12b-it-UD-Q4_K_XL.gguf"
  mv "$GGUF.part" "$GGUF"
fi

# 2) PIN: recusa divergente (mesma disciplina do repositório)
GOT=$(sha256sum "$GGUF" | cut -d' ' -f1)
if [ "$GOT" != "$EXPECTED_SHA" ]; then
  echo "FATAL: sha256 divergente do pin!" >&2
  echo "  esperado: $EXPECTED_SHA" >&2
  echo "  obtido : $GOT" >&2
  exit 2
fi
echo "[setup] pin do modelo OK"

# 3) llama-server prebuilt (conhece gemma4; tenta CUDA, cai p/ CPU se não achar)
if [ ! -x llm/llama-server ]; then
  mkdir -p llm && pushd llm >/dev/null
  echo "[setup] baixando llama-server b11671…"
  curl -L --retry 3 -o llm.zip \
    "https://github.com/ggml-org/llama.cpp/releases/download/b11671/llama-b11671-bin-ubuntu-x64.zip" \
    && unzip -oq llm.zip && chmod +x llama-server 2>/dev/null || true
  popd >/dev/null
fi
command -v nvidia-smi >/dev/null && nvidia-smi | head -12 || echo "[setup] SEM GPU visível (CPU fallback)"
echo "[setup] pronto. Servidor: bash serve.sh"
