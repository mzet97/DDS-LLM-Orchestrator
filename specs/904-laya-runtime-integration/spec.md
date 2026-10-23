# T904 — Especificação de requisitos (REQ-904-*)

Fontes: PROMPT MESTRE T904 (seção 4/§6), contracts.md §Routing.Decision, decision.rs, audit.md.
Requisitos são verificáveis; rastreio em `traceability.md`.

## REQ-904-001 — Checkpoint local explícito

O `RealLayaAdapter` deve aceitar `checkpoint_path` (diretório local completo ou caminho de
`model.safetensors`); com `checkpoint_path` presente, o load usa EXCLUSIVAMENTE esse artefato
(`laya.load(<path local>)`). O modo legado (ID remoto `convaiinnovations/laya`) continua
disponível apenas SEM `checkpoint_path`, para reprodução dos experimentos do BASE.
CLI: `--checkpoint-path` + `--checkpoint-sha256`.

## REQ-904-002 — Verificação SHA-256 antes do load

O sha256 de `model.safetensors` do artefato local deve ser calculado ANTES de `laya.load` e
comparado ao pin esperado. Divergência ⇒ `CheckpointHashMismatchError` (causa tipada, sem load);
arquivo ausente ⇒ `CheckpointMissingError`; diretório incompleto (faltando `rl_agent_config.json`,
`encoder/config.json`, `tokenizer/tokenizer_config.json`, `tokenizer/tokenizer.json`) ⇒
`CheckpointIncompleteError` listando os faltantes. Nenhum desses casos tenta download/fallback.

## REQ-904-003 — Nenhum fallback silencioso

No modo local é proibido: carregar `convaiinnovations/laya`, resolver alias do registry do SDK
ou acessar rede. O adapter deve operar com `HF_HUB_OFFLINE=1`/`TRANSFORMERS_OFFLINE=1` no
carregamento local e passar o CAMINHO para `laya.load` (nunca o ID). Testes provam que o
loader nunca recebe o ID remoto no modo local.

## REQ-904-004 — Identidade correta na decisão DDS

`LayaAnswer.model_id` = `local:<nome do diretório>` (ou `--model-id` explícito);
`LayaAnswer.checkpoint_revision` = o sha256 VERIFICADO do `model.safetensors` carregado (64 hex).
Esses valores fluem para `Routing.Decision.model_id` / `.checkpoint_revision` sem transformação.

## REQ-904-005 — Calibração ligada ao checkpoint

Calibração só é aplicável se vinculada ao checkpoint carregado. As calibrações v1–v4 existentes
estão vinculadas ao BASE `891102d3…` (campo `model_id_or_path`) — NÃO são aplicáveis ao T900.
O adapter aceita `--calibration-path` (pin = sha256 do arquivo); sem calibração vinculada ao
T900, `calibration_revision` segue vazia ⇒ decisão aceita SOMENTE em shadow (decision.rs).

## REQ-904-006 — Shadow sem Tasks

O worker permanece sem caminho de escrita em `Tasks` (fronteira estrutural de space.py).
Shadow real deve provar `tasks_seen=0` e `tasks_written=0` no observador (`wf-observe`).

## REQ-904-007 — E2E ativo somente após gates

Execução ativa (materialização de `Tasks laya1-…`) exige: M5 shadow aprovado E calibração
aceita pelo validator (`DecisionPolicy.calibration_pin` casando). Sem calibração aceita do
T900, o E2E ativo fica `BLOCKED`/`NOT_RUN` com evidência — não se contorna o gate.

## REQ-904-008 — Preservação T900–T903

Nenhum peso, relatório ou manifest de T900–T903 é sobrescrito/movido. A montagem runtime usa
symlinks para os artefatos canônicos (leitura apenas). Correções documentais (ex.: typo do
`decision.json` do T903) entram como notas datadas, nunca como reescrita.

## REQ-904-009 — Regressão dos contratos 19+5

Os 19 tópicos canônicos e os 5 tópicos `WorkflowPlanning` permanecem byte-idênticos
(digest vectors e abi-snapshots verdes); bindings Python regeneram determinístico
(hash pin do README do worker); `cargo test` (workflow-engine, dds-contract, agent,
dds-dataspace), clippy `-D warnings` e fmt verdes; caminho legado sem `--adaptive` intacto;
`--qos-manager static` segue default.

## REQ-904-010 — Documentação e reprodução

README do worker atualizado (runtime existe; receita de montagem e execução);
`workflow-engine` deixa de ser "estudo complementar" na doc oficial;
`REPORT.md`/`REPRODUCE.md`/`FINAL_HANDOFF.md` apenas com resultados observados nesta execução.
