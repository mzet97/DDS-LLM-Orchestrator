# workers/laya-worker — worker Laya (fase 900; runtime T904)

Worker Python de decisão Laya: assina `Workflow.Intent` e publica SOMENTE
`Routing.Decision` (fronteira estrutural — não publica `Tasks`, não executa
ferramentas, não concede permissão). A decisão do modelo é dado NÃO CONFIÁVEL
até o validator Rust (`crates/workflow-engine/src/decision.rs`).

**Estado T904 (2026-10-21): runtime REAL operacional em shadow.** CLI com
checkpoint LOCAL explícito — o `RealLayaAdapter` calcula o sha256 do
`model.safetensors` ANTES do `laya.load` (erros tipados: `CheckpointMissingError`,
`CheckpointIncompleteError`, `CheckpointHashMismatchError`), carrega OFFLINE
sem fallback para o ID remoto, e publica `model_id`/`checkpoint_revision`
do artefato verificado. Sem calibração vinculada ao checkpoint, a decisão
permanece em shadow (`calibration_revision=""`) — o validator Rust nunca
materializa `Tasks` nesse estado.

## Executar (shadow, T900 real)

```bash
# 1. Montar o diretório completo do checkpoint (symlinks zero-cópia + manifest)
python3 tools/assemble_checkpoint.py \
  --weights /var/mnt/HD1TB/tese/src/laya_t900_handoff/colab_results/models/laya_specialized_v1/model.safetensors \
  --complement ~/.cache/huggingface/hub/models--convaiinnovations--laya/snapshots/7b928d828b7b0e022f929d9bd2e44165aa270148 \
  --out ~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1 \
  --expected-sha256 358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f

# 2. Worker em domínio isolado (loopback em TODOS os processos do experimento)
CYCLONEDDS_URI=file://harness/cyclonedds_loopback.xml \
PYTHONPATH=src ~/.cache/tese-laya/venv/bin/python -m laya_worker \
  --domain 231 \
  --checkpoint-path ~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1 \
  --checkpoint-sha256 358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f
```

Flags: `--mock` (FakeLayaAdapter, offline), `--checkpoint-path` +
`--checkpoint-sha256` (LOCAL; sem path = modo legado BASE remoto pinado),
`--model-id`, `--calibration-path` (pin vira `calibration_revision`; SÓ com
calibração vinculada ao MESMO checkpoint), `--max-intents`, `--idle-timeout`.

Shadow end-to-end (worker + observador Rust + cenários):
`python3 harness/run_shadow_t904.py --domain 231` (gate: `tasks_seen=0`,
`tasks_written=0`).

## Bindings gerados (T-900-M1-02)

- Fonte única: `../../crates/dds-contract/idl/WorkflowPlanning.idl`.
- Gerado por `idlc -l py` (backend idlpy 11.0.1). O backend **ignora `-o`** e
  escreve `./dds_llm_workflow_v1/` no CWD.
- Regeneração: `regen_bindings.sh` (receita do backend `_idlpy` em
  `../../specs/900-laya-adaptive-routing/evidence/m1-01/README.md`).
- Hash pin (reprodutível — duas execuções geram bytes idênticos):
  - `_WorkflowPlanning.py` sha256 `19b0604f82b7678706c09a7bd5046ba51783931e53834bdfa6da33755e82741f`
  - `__init__.py` sha256 `9c6f11b973f2a669a91aa113608f019b1561f3a9235603781499ed67d9e30d60`

## Semântica de uso (emenda 3 do IDL)

Campos de enum são `int32` carregando o **ordinal** — passe
`m.QosIntent.QI_CRITICAL.value` (nunca o membro do enum: o pack falha).
`IdlEnum.auto()` aqui é 0-based, igual aos ordinais do IDL. Serialização:
`sample.serialize()` / `T.deserialize(bytes)` (métodos de instância/classe —
`IDL(T)` separado não popula a máquina da classe).

## Testes

```bash
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=src python3 -m pytest -p no:cacheprovider tests -q
```

Dossiê T904 (auditoria de checkpoints, calibração, evidências):
`../../specs/904-laya-runtime-integration/`.
