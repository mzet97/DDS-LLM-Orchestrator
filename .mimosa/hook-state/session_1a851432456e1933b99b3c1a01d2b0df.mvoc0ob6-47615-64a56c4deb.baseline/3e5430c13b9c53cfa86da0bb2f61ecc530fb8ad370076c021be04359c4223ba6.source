#!/usr/bin/env bash
# Sobe o llama-server (porta 8901; -np 1 — disciplina B-BENCH).
set -euo pipefail
cd "$(dirname "$0")"
exec llm/llama-server -m models/gemma-4-12b-it-UD-Q4_K_XL.gguf \
  -ngl 99 -np 1 -c 8192 --host 127.0.0.1 --port 8901
