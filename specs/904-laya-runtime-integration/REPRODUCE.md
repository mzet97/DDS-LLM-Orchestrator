# T904 — REPRODUCE (comandos realmente executados — 2026-10-21)

Checkout: `/var/mnt/HD1TB/tese/src/rust` (branch `studio/phase-800-node`, HEAD `fc56e60`).

```bash
# M0 — estado git
cd /var/mnt/HD1TB/tese/src/rust && git status --short --branch && git rev-parse HEAD

# M0 — hashes dos checkpoints (9 × model.safetensors; ver evidence/checkpoint_hashes_m0.txt)
sha256sum /var/mnt/HD1TB/tese/src/laya_t900_handoff/colab_results/models/laya_specialized_v1/model.safetensors

# M0 — baseline pytest (25 passed)
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=workers/laya-worker/src \
python3 -m pytest -p no:cacheprovider workers/laya-worker/tests -q

# M2/M3 — testes do adapter (34 passed após implementação)
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=workers/laya-worker/src \
python3 -m pytest -p no:cacheprovider workers/laya-worker/tests -q

# M3 — montar checkpoint T900 completo (symlinks; hash verificado)
python3 workers/laya-worker/tools/assemble_checkpoint.py \
  --weights /var/mnt/HD1TB/tese/src/laya_t900_handoff/colab_results/models/laya_specialized_v1/model.safetensors \
  --complement /home/mzet/.cache/huggingface/hub/models--convaiinnovations--laya/snapshots/7b928d828b7b0e022f929d9bd2e44165aa270148 \
  --out /home/mzet/.cache/tese-laya/checkpoints/laya-t900-specialized-v1 \
  --expected-sha256 358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f

# M3 — smoke real (carga + inferência + controle negativo; torch CPU)
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=workers/laya-worker/src \
/home/mzet/.cache/tese-laya/venv/bin/python - <<'PY'  # ver evidence/m3-smoke-real-t900.json
PY

# M5 — shadow real (requer wf-observe buildado; domínio 231; loopback OBRIGATÓRIO em todos)
export CARGO_TARGET_DIR=/tmp/tese-laya-t904-target CYCLONEDDS_STATIC=1
cargo build -p workflow-engine --features dds --locked
cd workers/laya-worker && python3 harness/run_shadow_t904.py --domain 231

# M7 — regressão
export CARGO_TARGET_DIR=/tmp/tese-laya-t904-target CYCLONEDDS_STATIC=1
cargo test -p dds-contract --locked; cargo test -p workflow-engine --locked
cargo test -p agent --locked; cargo test -p dds-dataspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings   # gate do CI
cargo fmt --all -- --check
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=workers/laya-worker/src \
python3 -m pytest -p no:cacheprovider workers/laya-worker/tests -q

# M7 — preservação (re-hash dos 9 pesos; conferir com evidence/checkpoint_hashes_m0.txt)
sha256sum /var/mnt/HD1TB/tese/src/laya_t900_handoff/colab_results/models/*/model.safetensors \
          /var/mnt/HD1TB/tese/src/laya_t900_handoff/t90{2,3}/models/arm_*/model.safetensors
```

Observações de ambiente: (a) `CYCLONEDDS_URI=file://…/cyclonedds_loopback.xml` deve estar no
ambiente de TODOS os processos DDS (incluindo o publicador Python), senão o tráfego vaza para
a LAN e os transportes não casam; (b) build/DDS no SMB usa target dir exclusivo em /tmp;
(c) torch do venv é CPU-only — latência ~3 s/decisão (13,8 s na 1ª com warmup).
