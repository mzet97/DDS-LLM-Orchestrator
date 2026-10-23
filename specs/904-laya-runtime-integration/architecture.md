# T904 — Arquitetura do carregamento e execução (estado REAL)

## Fluxo alvo (T900 no runtime)

```
[montagem M3, uma vez]
pesos T900 (SMB) ──symlink──┐
BASE snapshot (HF cache) ───┤→ ~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1/
                            │    ├ model.safetensors          → T900 (35813054…)
                            │    ├ rl_agent_config.json       → BASE (arquitetura, não treinado)
                            │    ├ encoder/config.json        → BASE
                            │    └ tokenizer/*.json           → BASE
                            └→ MANIFEST.assembly.json (hashes de tudo)

[execução]
wf client → Workflow.Intent(checkpoint_revision="35813054…")
   → laya-worker (RealLayaAdapter, --checkpoint-path ... --checkpoint-sha256 35813054…)
       1. resolve diretório; verifica arquivos exigidos pelo SDK (CheckpointIncompleteError se faltar)
       2. sha256(model.safetensors) ANTES do load → diverge ⇒ CheckpointHashMismatchError (sem load)
       3. HF_HUB_OFFLINE=1; laya.load("<dir local>") — sem registry, sem rede (laya==0.4.10
          trata path com separador como checkpoint local; expected_sha256 nativo como 2ª defesa)
       4. decide() → LayaAnswer(model_id="local:laya-t900-specialized-v1",
                                 checkpoint_revision="35813054…",
                                 calibration_revision=<pin ou "">)
   → Routing.Decision
   → validator Rust (decision.rs): correlação + revisões (intent pin 35813054… deve casar)
       calibration_pin None ou ≠ ⇒ Accept{shadow_only=true} — NUNCA materializa
   → wf-observe: contadores (tasks_seen=0, tasks_written=0)

[E2E ativo — SOMENTE com calibração T900 aceita]
Routing.Decision(calibration_revision=pin) + DecisionPolicy.calibration_pin=Some(pin)
   → Accept{shadow_only=false} → mapper QoS → planner → DAG → journal SQLite
   → Workflow.Plan/State → Task laya1-… → agente --adaptive → engine → resultado
```

## O que muda no código (mínimo)

| Arquivo | Mudança |
|---|---|
| `workers/laya-worker/src/laya_worker/adapter.py` | `RealLayaAdapter(checkpoint_path=…, checkpoint_sha256=…, model_id=…)`: erros tipados (`CheckpointMissingError`, `CheckpointIncompleteError`, `CheckpointHashMismatchError` — subclasses de `LayaUnavailableError`), verificação pré-load, load local offline, loader injetável (`_laya_load`) para testes; legado preservado sem `checkpoint_path` |
| `workers/laya-worker/src/laya_worker/cli.py` | flags `--checkpoint-path`, `--checkpoint-sha256` (obrigatório com path), `--model-id` |
| `workers/laya-worker/tools/assemble_checkpoint.py` | NOVO: monta diretório completo por symlinks + `MANIFEST.assembly.json` (hashes), idempotente, zero-cópia |
| `workers/laya-worker/tests/test_adapter.py` | testes M2 (fixtures pequenos, loader fake injetado) |
| `workers/laya-worker/README.md` + docs do workflow-engine (M8) | atualização documental |

NÃO muda: IDL, bindings gerados, `questions.py`, `decide.py`, `space.py`, validator Rust,
mapper QoS, 19 tópicos canônicos, `qos-manager static`.

## Defesa em profundidade (sem fallback silencioso)

1. Caminho explícito na CLI (não há default de path).
2. Hash pré-load no adapter (erro tipado; o loader injetável prova que `laya.load` não é chamado).
3. `HF_HUB_OFFLINE=1`/`TRANSFORMERS_OFFLINE=1` setados no load local.
4. Path absoluto com separador ⇒ SDK trata como local (`_is_local_checkpoint_arg`), sem registry.
5. `expected_sha256` nativo do SDK repassado com o hash do manifest (2ª verificação).
6. Identidade na decisão = hash verificado; validator Rust casa com o pin da INTENÇÃO.

## Calibração (estado)

- v1–v4 existentes → BASE. T900 → sem calibração ⇒ `calibration_revision=""` ⇒ shadow_only=true
  (gate estrutural do validator). Recalibração T900: opção registrada (CPU ~2,9 s/decisão;
  12k train do SPROUT ≈ 10 h CPU; subsets menores proporcionais) — só com autorização.
