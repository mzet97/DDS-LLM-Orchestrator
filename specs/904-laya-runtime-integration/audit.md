# T904 — Auditoria M0 (somente leitura) — 2026-10-21

Estado: **GATE M0 PASS** (T900 presente com hash exato; nenhuma alteração de código nesta fase).

## 1. Estado do checkout

- Repo Git do workspace Rust: `/var/mnt/HD1TB/tese/src/rust` (a raiz `/var/mnt/HD1TB/tese` NÃO é repo Git — confirmado).
- Branch `studio/phase-800-node`, HEAD `fc56e60daa22b792dc990df9d27e83bc6763e28c` (igual ao citado no HANDOFF 900).
- Working tree (preservar, NÃO reverter):
  - Modificados: `README.md` (raiz do workspace), `specs/900-laya-adaptive-routing/README.md`, `__pycache__` do worker.
  - **Não versionados** (apenas working tree): `specs/900-laya-adaptive-routing/CONFERENCIA_IMPLEMENTACAO_X_DOC_LAYA.md`,
    `specs/900-laya-adaptive-routing/HANDOFF_LAYA_IMPLEMENTACAO.md`, `specs/902-laya-data-centric-training/`, `specs/903-laya-streaming-generalization/`.
- Evidência completa: `evidence/git_state_m0.txt`.
- `laya_t900_handoff/` está em `/var/mnt/HD1TB/tese/src/laya_t900_handoff/` (fora do repo Git, ao lado de `rust/`) — AVAILABLE.

## 2. Inventário de checkpoints T900–T903

Matriz completa com hashes nesta execução: `evidence/checkpoint_hashes_m0.txt` e `checkpoint_registry.md`.

| Etapa | Estado local | Hash verificado | Científico |
|---|---|---|---|
| BASE `convaiinnovations/laya` | AVAILABLE (cache HF, snapshot `7b928d82…`, 842609210 B) | `891102d3…` = pin do adapter | BASE |
| T900 auxiliar | AVAILABLE (`colab_results/models/laya_taskbench_aux/`, só pesos) | `030f34b5…` | preservado (intermediário) |
| **T900 especializado (VIGENTE)** | AVAILABLE (`colab_results/models/laya_specialized_v1/`, **só `model.safetensors`**) | **`35813054…` EXATO** | VIGENTE |
| T901 braços A–D | **AUSENTE** (pesos na VM Colab; hashes/relatórios em `t901/`) | ver `t901/manifest_t901.json` | KEEP_T900 |
| T902 braços A/B/C | AVAILABLE (`t902/models/arm_{A,B,C}/`, só pesos) | `f8616426…`/`5d7dd652…`/`b03a5f82…` | REJEITADO (arm_B finalista) |
| T903 braços A/B/C | AVAILABLE (`t903/models/arm_{A,B,C}/`, só pesos) | `b2d9d950…`/`ccb50b39…`/`7de213c7…` | REJEITADO (arm_C finalista) |

Restrição física de todos os pesos treinados locais: **apenas `model.safetensors`** — os diretórios
completos (rl_agent_config.json, encoder/, tokenizer/, questions.json) ficaram na VM Colab
(manifest T902 lista `models/arm_X/checkpoint_latest/encoder/config.json` etc.).

## 3. Divergência principal — CONFIRMADA no código

`workers/laya-worker/src/laya_worker/adapter.py` (linhas 89–172), versão atual:

```python
CHECKPOINT_SNAPSHOT = "7b928d828b7b0e022f929d9bd2e44165aa270148"
CHECKPOINT_MODEL_SHA256 = "891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c"
MODEL_ID = "convaiinnovations/laya"
...
self._agent = laya.load(self.MODEL_ID)   # carrega PELO ID REMOTO (cache HF)
```

- O `RealLayaAdapter` pinna e verifica o **BASE** `891102d3…` e carrega por `laya.load("convaiinnovations/laya")`.
- **Conclusão: o runtime carrega o checkpoint BASE, não o T900 treinado.** Esta é a falha que a T904 corrige.
- O globo do cache HF só VERIFICA (se o snapshot existir); nunca aponta o `laya.load` para o T900.

## 4. Calibração — nenhuma vinculada ao T900

`specs/900-laya-adaptive-routing/evidence/calibration/`:

| Versão | Pin (sha256 do JSON) | Vinculada a |
|---|---|---|
| v1 (snapshot400) | `a235c9b9…` | **BASE** (`model_id_or_path: convaiinnovations/laya`) |
| v2 | `f7451ac7…` | BASE |
| v3 | `a1ff7ddd…` | BASE |
| **v4 (SPROUT real)** | `10ef5945…` (gates_v4) | **BASE** (`calibration_v4.json`: `model_id_or_path: convaiinnovations/laya`) |

- **Não existe calibração para o T900 `35813054…`.** Aplicar a do BASE ao T900 é proibido (M4).
- venv do SDK: `/home/mzet/.cache/tese-laya/venv` — `laya==0.4.10`, `torch 2.14.1+cpu` (**sem GPU local**);
  latência CPU p50 ≈ 2,9–3,6 s/decisão (reports de calibração) → recalibração completa em CPU custa horas.
- Consequência (ver plan.md): **estado SHADOW_ONLY é o resultado honesto do M4**, com recalibração
  T900 como opção registrada (custo estimado) e E2E ativo (M6) condicionado.

## 5. Código examinado (fluxo real)

- `workers/laya-worker/src/laya_worker/adapter.py` — `FakeLayaAdapter` (regras) e `RealLayaAdapter`
  (SDK LAZY; verifica pin no globo do cache; `laya.load(MODEL_ID)`; timeout por deadline; `LayaAnswer`
  com `model_id`/`checkpoint_revision`/`calibration_revision`).
- `cli.py` — flags `--mock`, `--checkpoint-sha256`, `--calibration-path`; imprime `{"shadow": stats, "tasks_written": 0}`.
- `space.py` — assina `Workflow.Intent`, publica SOMENTE `Routing.Decision` (fronteira estrutural).
- `decide.py` — erros viram `DS_ERROR` com reason tipado; `payload_hash` exclui o próprio campo;
  `calibration_revision` vazia = shadow.
- `questions.py` — `QUESTIONS_REVISION = 1`, `QUESTIONS_V1` intocado (não mudar).
- Rust: `crates/workflow-engine/src/decision.rs` — validator puro: correlação, revisões pinadas
  (inclusive `intent.checkpoint_revision`), janela de validade, distribuições, catálogo;
  `DecisionPolicy.calibration_pin: Some(pin)` exige match para sair de shadow; sem pin ⇒ sempre shadow.
- `crates/workflow-engine/src/bin/wf_observe.rs` — observador com contadores `tasks_seen`/`tasks_written` (gate V04).
- SDK `laya==0.4.10` (`agent.py`): `laya.load()` aceita DIRETÓRIO local (`_is_local_checkpoint_arg`:
  path com separador ⇒ local, sem registry/rede); exige no diretório `rl_agent_config.json`,
  `model.safetensors`, `tokenizer/*`, `encoder/*` (erro tipado se faltar); tem verificação NATIVA
  `expected_sha256: {caminho_relativo: hexdigest}`; `load_calibration(path)` para temperatura.

## 6. Baseline de testes (baratos, sem rede) — nesta execução

```
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=workers/laya-worker/src \
python3 -m pytest -p no:cacheprovider workers/laya-worker/tests -q
→ 25 passed
```

## 7. NOT_AVAILABLE / NOT_RUN (M0)

- Pesos T901 A–D: **NOT_AVAILABLE** localmente (VM Colab) — hashes/relatórios preservados.
- Nenhum teste com pesos reais foi rodado no M0 (fora do escopo da fase somente leitura).

## 8. Decisões da auditoria que a T904 assume

1. T900 local `35813054…` é o artefato canônico a carregar (hash EXATO conferido nesta execução).
2. O diretório do T900 é incompleto ⇒ a T904 precisa de **montagem de diretório completo**
   (pesos T900 + complemento arquitetural do BASE: `rl_agent_config.json`, `encoder/config.json`,
   `tokenizer/tokenizer_config.json`, `tokenizer/tokenizer.json`) com hashes registrados —
   o SDK exige esses arquivos e NÃO carrega só-pesos.
3. Montagem em FS local (`~/.cache/tese-laya/`) por symlinks zero-cópia (SMB com 22 G livres, 98%).
4. `model_id` da decisão local: `local:laya-t900-specialized-v1`; `checkpoint_revision`: sha256 do
   model.safetensors verificado no load (64 hex, cabe em `Digest` string<64> do contrato).
5. Calibração T900 não existe ⇒ SHADOW_ONLY no primeiro ciclo; recalibração local CPU registrada
   como opção com custo (não executada sem autorização de horas de máquina).
