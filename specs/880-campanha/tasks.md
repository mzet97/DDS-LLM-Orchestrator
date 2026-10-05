# specs/880-campanha — Campanha confirmatória (EM ANDAMENTO)

Fonte: `PLANO_IMPLEMENTACAO_RESTANTE.md` Fase 880. Protocolo: dissertação Tab.11 + manifesto de congelamento (`specs/860-backends/manifest-freeze.md`). Ordem: EXP0 → EXP1 → EXP1a → EXP1b → EXP2 → EXP3 → EXP4 → EXP-C → análise.

## Pré-requisitos executados (2026-10-05)
- **llama-server .61 religado**: `build-vm/bin/llama-server` + modelo congelado `Qwen3.5-0.8B-Q4_K_M.gguf` (SHA `bd258782…`), porta 8082, health 200 (pré-requisito do EXP2).
- **Agentes confirmados no domínio 170**: `agent-61` (.61), `agent-64a`/`agent-64b` (.64), `agent-63` (.63) no registry do orquestrador .62.
- Venv do harness recriada em FS local (`~/.cache/tese-venv-exp0`, 233 pacotes do lock congelado; SMB não suporta symlinks da venv).

## EXP0 — confirmatório: **✅ PASSOU** (`exp0/`)
- **5 runs × 3 sistemas + DDS**: MAF 5 runs, LangGraph 5 runs (harness, backend stub), DDS 5 runs (wf-run com prompts congelados → agent engine=dds → det-responder; 25/25 workflows).
- **Oráculo** (`bench.validation.oracle_exp0` → `exp0/oracle.md`): **85 fontes (17 por caso × 5 casos), 0 conflitos — BYTE-IDÊNTICO** entre MAF/LangGraph/DDS, **incluindo os pilotos de 2026-09-10** (reprodutibilidade entre épocas e hosts).
- Manifest por fase: `exp0/manifest.json`; artefatos DDS: `exp0/dds-run*-case*.json` + `det-responder.jsonl`; runs Python: `results/*-n5-*` (manifest.json por run).
- Lição de execução: o glob do DDS saiu errado na 1ª passada do oráculo (comparou só MAF/LG/pilotos) — **detectado e corrigido** (2ª passada com 17 fontes/caso).

## EXP1 — overhead: **pendente** (destravado pela calibração 860)
## EXP1a — ablação InMemory vs DDS: **pendente** (mesmo binário Rust)
## EXP1b — ablação claim vs despacho central: **pendente** (requer `--dispatch-mode`, pequeno)
## EXP2 — LLM real: **pendente** (llama .61 no ar; agente .61 troca engine para http→8082)
## EXP3 — concorrência c∈{1,2,4,8}: **pendente** (crash não-reproduzível pós-850)
## EXP4 — falha/recuperação: **pendente** (T̃ref do piloto independente a congelar antes)
## EXP-C — contenção k∈{2,4,8}: **pendente** (agentes .61+.64)
