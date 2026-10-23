# T904 — FINAL_HANDOFF (2026-10-21)

## Estado final

O `laya-worker` carrega agora, de forma explícita e verificável, o checkpoint **T900
`358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f`** por `--checkpoint-path`
(hash calculado ANTES do load; erros tipados; offline; sem fallback para o ID remoto) e
publica a identidade correta em `Routing.Decision`. O fluxo foi provado em **shadow DDS real**
(`tasks_seen=0`, `tasks_written=0`, calibração divergente nunca sai de shadow). O **E2E ativo
permanece bloqueado** — não existe calibração vinculada ao T900 (v1–v4 são do BASE) e o gate
não pode ser contornado.

## Artefatos de código (únicos tocados — todos em workers/laya-worker/)

| Arquivo | Mudança |
|---|---|
| `src/laya_worker/adapter.py` | modo local (`checkpoint_path`/`model_id`), erros `CheckpointMissing/Incomplete/HashMismatch` (subclasses de `LayaUnavailableError`), hash pré-load em stream, `_load_local` offline, loader injetável `_laya_load`, `CHECKPOINT_REQUIRED_FILES`; modo legado do BASE preservado |
| `src/laya_worker/cli.py` | `--checkpoint-path`, `--model-id`, pareamento obrigatório com `--checkpoint-sha256` |
| `tools/assemble_checkpoint.py` | NOVO — monta diretório completo (symlinks zero-cópia) + `MANIFEST.assembly.json` |
| `tests/test_adapter.py` | +9 testes T-904-M2 (34 total) |
| `harness/run_shadow_t904.py` | NOVO — shadow real com 8 cenários + wf-observe |
| `README.md` | estado real do runtime + receitas |

Nenhuma mudança em: IDL, bindings, `questions.py`, `decide.py`, `space.py`, Rust (zero `.rs`),
mapper QoS, CI. Specs 902/903 e HANDOFF 900 (não versionados) preservados.

## Checkpoint operacional

- Diretório: `~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1/` (symlinks; pesos
  canônicos em `src/laya_t900_handoff/colab_results/models/laya_specialized_v1/`).
- Manifest: `MANIFEST.assembly.json` no diretório (hashes de todos os 5 arquivos).
- Identidade publicada: `model_id=local:laya-t900-specialized-v1`,
  `checkpoint_revision=35813054…`, `calibration_revision=""` (SHADOW_ONLY).

## Próxima ação exata (T905)

1. Definir split de recalibração do T900 ANTES de rodar (SPROUT derivado, seed registrada;
   nunca os selados T902 `db457693…`/T903 `7578942a…`; preferir split novo, não o v4_test que
   foi holdout do BASE).
2. Autorizar custo (CPU local ~50 min/1k decisões; ou GPU externa) e rodar
   `tools/calibrate_laya.py <dataset> <outdir> --seed 900`.
3. Registrar pin no `checkpoint_registry.md` + gates; reexecutar
   `harness/run_shadow_t904.py` com `--calibration-pin <novo pin>` (esperado:
   `shadow_only=false` para decisões com o pin) e então M6 (DirectTemplatePlanner + mock, 11 cenários).
4. Opcional: corrigir o bin `dag_validate` para clippy default-features (observação
   pré-existente; fora do escopo T904).

## Como reproduzir

`REPRODUCE.md` (comandos executados) · `REPORT.md` (resultados) · `evidence/` (logs, hashes,
manifests). Nada disto exige acesso à VM Colab.
