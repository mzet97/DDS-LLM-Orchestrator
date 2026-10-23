# specs/904-laya-runtime-integration — T904

**Objetivo:** fazer o `laya-worker` carregar, de forma explícita e verificável, o checkpoint
T900 `358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f` (hoje o runtime carrega
o BASE `891102d3…`), transportar a identidade correta em `Routing.Decision`, provar o fluxo em
shadow e — só com calibração aceita — o caminho ativo.

**Estado:** M0/M1 concluídos (auditoria + spec). Gate M0 PASS (T900 local com hash EXATO).

## Quickstart (após M3)

```bash
# 1. Montar o diretório completo do T900 (symlinks zero-cópia + manifest)
python3 workers/laya-worker/tools/assemble_checkpoint.py \
  --weights /var/mnt/HD1TB/tese/src/laya_t900_handoff/colab_results/models/laya_specialized_v1/model.safetensors \
  --complement ~/.cache/huggingface/hub/models--convaiinnovations--laya/snapshots/7b928d828b7b0e022f929d9bd2e44165aa270148 \
  --out ~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1 \
  --expected-sha256 358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f

# 2. Worker em shadow com o T900 real
CYCLONEDDS_URI=file://workers/laya-worker/harness/cyclonedds_loopback.xml \
PYTHONPATH=workers/laya-worker/src ~/.cache/tese-laya/venv/bin/python -m laya_worker \
  --domain 231 --checkpoint-path ~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1 \
  --checkpoint-sha256 358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f
```

## Documentos

`audit.md` (M0) · `checkpoint_registry.md` · `spec.md` (REQ-904-*) · `architecture.md` ·
`plan.md` · `tasks.md` · `risks.md` · `traceability.md` · `evidence/` ·
`REPORT.md`/`REPRODUCE.md`/`FINAL_HANDOFF.md` (no fim da execução).
