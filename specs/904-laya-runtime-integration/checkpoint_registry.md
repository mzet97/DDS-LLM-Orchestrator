# Registro de checkpoints Laya — T904 (fonte de verdade operacional)

> Atualizado: 2026-10-21 (hashes verificados nesta execução; ver `evidence/checkpoint_hashes_m0.txt`).
> Este registro NÃO substitui os relatórios científicos T900–T903; apenas cataloga artefatos e estados.

## Estados

`BASE` · `VIGENTE` · `REJEITADO` · `AUSENTE` (pesos não disponíveis localmente) · `NÃO VERIFICADO`.

## Checkpoints

| ID | Artefato | SHA-256 (model.safetensors) | Bytes | Localização | Estado | Diretório completo? |
|---|---|---|---|---|---|---|
| base | `convaiinnovations/laya` | `891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c` | 842609210 | cache HF `snapshots/7b928d82…` (local) | BASE | SIM (cache HF) |
| t900-aux | `laya_taskbench_aux` | `030f34b5f417af18af94a30e62b3cbd3aa2ec1924e483fb82484dbb3a9f1f84e` | 842609220 | `src/laya_t900_handoff/colab_results/models/` | preservado (intermediário do T900) | NÃO (só pesos) |
| **t900 (VIGENTE)** | `laya_specialized_v1` | **`358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f`** | 842609220 | idem | **VIGENTE** | NÃO (só pesos) |
| t901-a/b/c/d | braços A–D | ver `laya_t900_handoff/t901/manifest_t901.json` | — | VM Colab (não local) | AUSENTE (rejeitados) | — |
| t902-A | arm_A | `f86164262d428db56903fc6546ff378b26c886c355020cac8bf85610930eade3` | 842609220 | `src/laya_t900_handoff/t902/models/` | REJEITADO | NÃO (só pesos) |
| t902-B | arm_B (finalista) | `5d7dd6522d94347f47566499e09cbd2ad90b4fccbeb26a91c6885fa8c0eda4c0` | 842609220 | idem | REJEITADO (decision: KEEP_T900) | NÃO (só pesos) |
| t902-C | arm_C | `b03a5f82cb3c8fc34be23dcc0ff2511f531a0aa795f528cd0463a7d22fee2b6a` | 842609220 | idem | REJEITADO | NÃO (só pesos) |
| t903-A | arm_A | `b2d9d950a46ef7ae57669b6cfbce41c577f1e423d310d7299f189be6859f3b50` | 842609220 | `src/laya_t900_handoff/t903/models/` | REJEITADO | NÃO (só pesos) |
| t903-B | arm_B | `ccb50b391d698d6b84a13a00a5d6936259800cd6c79b93a5d6f91cc30f025e81` | 842609220 | idem | REJEITADO | NÃO (só pesos) |
| t903-C | arm_C (finalista) | `7de213c7b24fca603f1f58e04ad4f8029fef396d5cf0e24e3c0067bb996cd05d` | 842609220 | idem | REJEITADO (gate FAIL) | NÃO (só pesos) |

## Calibrações

| Versão | Pin (sha256 do JSON) | Vinculada ao checkpoint | Dataset |
|---|---|---|---|
| v1 | `a235c9b9…` | BASE | sintético gemma4 (400) |
| v2 | `f7451ac7…` | BASE | sintético (840) |
| v3 | `a1ff7ddd…` | BASE | sintético (1280) |
| v4 | `10ef5945…` (gates_v4 `pin_sha256`) | BASE | SPROUT real 12k train/2k holdout (144k records) |
| **t900-calib** | — | **T900 `35813054…`** | **NÃO EXISTE** (recalibração = opção registrada, custo CPU) |

## Montagem runtime do T900 (criada pela T904/M3)

Diretório operacional (zero-cópia, symlinks; complemento arquitetural do BASE):

- `~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1/`
  - `model.safetensors` → `src/laya_t900_handoff/colab_results/models/laya_specialized_v1/model.safetensors` (hash `35813054…` verificado ANTES do load)
  - `rl_agent_config.json`, `encoder/config.json`, `tokenizer/tokenizer_config.json`, `tokenizer/tokenizer.json` → blobs do snapshot BASE `7b928d82…` (componentes não-treináveis de arquitetura/tokenização, idênticos entre base e fine-tune)
  - Manifest da montagem com todos os hashes: `MANIFEST.assembly.json` no diretório + cópia em `evidence/`.

`model_id` na decisão: `local:laya-t900-specialized-v1` · `checkpoint_revision`: `35813054…` (64 hex).
