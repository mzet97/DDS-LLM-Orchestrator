# T904 — Tarefas atômicas (Given/When/Then)

## T-904-M2 — Testes falhando do adapter (antes de editar o adapter)

- [ ] **T-904-M2-01** — aceitar artefato local com hash correto
  GIVEN diretório de teste com `model.safetensors` (fixture 128 B, hash H) e complemento mínimo,
  WHEN `RealLayaAdapter(checkpoint_path=d, checkpoint_sha256=H)` com loader fake injetado decide,
  THEN decisão OK com `model_id="local:<nome>"` e `checkpoint_revision=H`, e o loader recebeu o PATH.
- [ ] **T-904-M2-02** — rejeitar hash incorreto
  GIVEN mesmo diretório com pin ≠ hash real, WHEN constrói+decide,
  THEN `CheckpointHashMismatchError` e o loader NÃO é chamado (sem load, sem rede).
- [ ] **T-904-M2-03** — rejeitar arquivo ausente
  GIVEN `checkpoint_path` inexistente, WHEN constrói+decide, THEN `CheckpointMissingError`.
- [ ] **T-904-M2-04** — rejeitar diretório incompleto
  GIVEN diretório só com `model.safetensors` (estado real do T900 baixado),
  WHEN constrói+decide, THEN `CheckpointIncompleteError` lista `rl_agent_config.json`,
  `encoder/config.json`, `tokenizer/tokenizer_config.json`, `tokenizer/tokenizer.json`.
- [ ] **T-904-M2-05** — sem rede e sem ID remoto no modo local
  GIVEN modo local com loader espião, WHEN decide, THEN loader nunca recebe `convaiinnovations/laya`
  e envs offline (`HF_HUB_OFFLINE`/`TRANSFORMERS_OFFLINE`) ficam ativos durante o load.
- [ ] **T-904-M2-06** — identidade do checkpoint real na resposta
  GIVEN loader fake que devolve respostas cruas, WHEN decide, THEN `LayaAnswer.model_id`/`.checkpoint_revision`
  vêm do artefato verificado (não do ID remoto).
- [ ] **T-904-M2-07** — import LAZY preservado
  GIVEN import de `laya_worker.adapter`, THEN `laya` não está em `sys.modules`;
  construção do adapter não importa SDK (só `_load`/`decide`).
- [ ] **T-904-M2-08** — calibração ligada ao arquivo carregado
  GIVEN `--calibration-path` apontando arquivo C, WHEN decide, THEN `calibration_revision=sha256(C)`;
  sem path, permanece "" (shadow).

## T-904-M3 — Carregamento explícito

- [ ] **T-904-M3-01** — `tools/assemble_checkpoint.py` monta diretório completo (symlinks, manifest
  com hashes, idempotente; `--expected-sha256` obrigatório).
- [ ] **T-904-M3-02** — montagem REAL do T900 em `~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1`.
- [ ] **T-904-M3-03** — smoke de carga real: `laya.load(dir)` + 1 inferência de validação em CPU
  (sem DDS), provando checkpoint `35813054…`; hash divergente (fixture) falha controlada.
- [ ] **T-904-M3-04** — CLI com as novas flags; `--checkpoint-sha256` exigido junto de `--checkpoint-path`.

## T-904-M4 — Calibração

- [ ] **T-904-M4-01** — manifesto `evidence/calibration_manifest.md`: v1–v4 → BASE; T900 sem calibração;
  opção de recalibração com custo estimado (CPU: ~2,9 s/decisão ⇒ 1k ≈ 50 min; 12k ≈ 10 h).
- [ ] **T-904-M4-02** — estado operacional declarado: **SHADOW_ONLY** (gate M4 satisfeito na 2ª forma).

## T-904-M5 — Shadow real (domínio DDS isolado, CYCLONEDDS_URI comum)

- [ ] **T-904-M5-01..07** — cenários: decisão válida; checkpoint divergente; calibração divergente;
  decisão vencida; revisão incorreta; timeout; entrada inválida — cada um com evidência
  (`evidence/m5-*.json`: contadores do wf-observe + decisões capturadas).

## T-904-M6 — E2E ativo (condicionado)

- [ ] **T-904-M6-01** — com calibração aceita: 11 cenários do prompt (DirectTemplatePlanner + mock primeiro).
- [ ] **T-904-M6-02** — sem calibração aceita (esperado): registrar `BLOCKED`/`NOT_RUN` com a causa
  (REQ-904-005/007) — sem contornar gate.

## T-904-M7 — Regressão (cargo 4 crates + clippy + fmt + pytest; DDS em /tmp target exclusivo)

## T-904-M8 — Documentação e handoff (README worker; doc workflow-engine; REPORT/REPRODUCE/FINAL_HANDOFF;
nota datada sobre typo do decision.json T903; registry atualizado)
