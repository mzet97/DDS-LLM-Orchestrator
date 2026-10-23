# T904 — REPORT (somente resultados observados nesta execução — 2026-10-21)

Legenda: **[VERIFICADO]** = executado/observado nesta sessão · **[HISTÓRICO]** = evidência
pré-existente citada · **[NÃO EXECUTADO]** / **[BLOQUEADO]** = estados honestos.

## Resposta às 10 perguntas do prompt

1. **Qual checkpoint o worker carregou de fato?** [VERIFICADO] ANTES da T904: o BASE
   `convaiinnovations/laya` (`891102d3…`) — divergência §2.3 CONFIRMADA no código. DEPOIS da
   T904: o T900 `35813054…` via `--checkpoint-path` (modo local explícito), provado no smoke
   real e no shadow DDS (`evidence/m3-smoke-real-t900.json`, `evidence/m5-shadow-real.json`).
2. **Qual SHA-256 foi verificado?** [VERIFICADO] `358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f`
   — calculado ANTES do load no `__init__` do adapter, no `assemble_checkpoint.py` e re-verificado
   pelo `expected_sha256` nativo do SDK. Hash divergente ⇒ `CheckpointHashMismatchError` sem load
   (controle negativo executado).
3. **Qual calibração foi usada e a qual checkpoint pertence?** [VERIFICADO] NENHUMA para o
   runtime T900: v1–v4 pertencem ao BASE (`evidence/calibration_manifest.md`). Runtime seguiu
   `calibration_revision=""` ⇒ 100% shadow. Estado: **SHADOW_ONLY** (Gate M4, forma explícita).
4. **O shadow passou com `tasks_seen=0` e `tasks_written=0`?** [VERIFICADO] SIM — DDS real,
   domínio 231 loopback: worker `tasks_written=0`; wf-observe `tasks_seen=0`, 4 accept (todas
   `shadow_only=true`), 4 reject com causa tipada, exit 0 (`evidence/m5-shadow-real.json`).
5. **O E2E ativo foi executado? Com qual engine?** [BLOQUEADO/NOT_RUN] NÃO — gate M6 exige
   calibração aceita do T900, que não existe; contornar é proibido (§8). Motivo e rota de
   destravamento: `evidence/m6-e2e-bloqueado.md`.
6. **Quais testes passaram / não executados?** [VERIFICADO] pytest worker **34/34** (25 pré-
   existentes + 9 novos T904); cargo test 4 crates **117 passed / 0 failed**; clippy CI-style
   (`--workspace --all-targets --all-features -D warnings`) **limpo**; `fmt --check` limpo.
   [NÃO EXECUTADO] DDS real dos testes Rust (`--features dds` inteiros do workspace) e campanha
   EXP; [BLOQUEADO] E2E ativo M6 (calibração).
7. **Os 19 tópicos antigos e os 5 novos permaneceram intactos?** [VERIFICADO] SIM — nenhum
   `.rs`/IDL/`Cargo.toml` modificado (`git status` limpo para `*.rs`; digest fixtures dentro dos
   117 testes de dds-contract verdes); tests de workflow-engine (validator, decision) verdes.
8. **Onde estão logs, manifests e hashes?** `specs/904-laya-runtime-integration/evidence/`
   (índice no fim deste arquivo) + `~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1/MANIFEST.assembly.json`.
9. **O que ficou bloqueado?** E2E ativo (M6) — única calibração existente é do BASE;
   recalibração do T900 é opção registrada com custo (CPU ~1–12 h conforme N; GPU externa
   autorização). Observação pré-existente registrada: clippy `--workspace` em default-features
   falha no HEAD em `dag_validate` (E0277, unificação de features `dds-contract/dds`) — NÃO é
   gate do CI (CI usa `--all-features`, que passa limpo) e NÃO foi introduzida pela T904
   (nenhum `.rs` tocado; `evidence/m7-preexisting-clippy-dag_validate.md`).
10. **Qual é a próxima tarefa exata?** T905 — recalibração do T900 em split SPROUT independente
    (definir split + registrar pin ANTES de rodar), depois re-shadow com `--calibration-pin` e
    destravar M6 (DirectTemplatePlanner + engine mock, 11 cenários).

## Marcos

| Marco | Resultado |
|---|---|
| M0 auditoria | **PASS** — T900 local com hash EXATO; inventário T900–T903 completo (T901 pesos só na VM) |
| M1 spec | CONCLUÍDO — 10 requisitos REQ-904-001..010 |
| M2 testes falhando | PASS — falharam pela causa esperada (API ausente), depois verdes |
| M3 carregamento | **PASS** — modo local + erros tipados + montagem zero-cópia; smoke real 7,2 s/decisão 13,8 s CPU |
| M4 calibração | **SHADOW_ONLY explícito** — manifesto + custo de recalibração |
| M5 shadow real | **PASS** — 8 cenários DDS; zero Tasks; calibração divergente nunca sai do shadow |
| M6 E2E ativo | **BLOQUEADO (NOT_RUN)** — sem calibração aceita do T900 |
| M7 regressão | VERDE nos gates do projeto (pytest 34, cargo 117, clippy all-features, fmt); 1 observação pré-existente registrada |
| M8 documentação | CONCLUÍDO — README worker, doc workflow-engine, registry, REPORT/REPRODUCE/FINAL_HANDOFF, nota datada typo T903 |

## Índice de evidências

- `evidence/git_state_m0.txt` — branch/HEAD/working tree no início (fc56e60)
- `evidence/checkpoint_hashes_m0.txt` — matriz SHA-256 dos 9 pesos (M0)
- `evidence/m3-smoke-real-t900.json` — carga real do T900 + controle negativo de hash
- `evidence/calibration_manifest.md` — binding v1–v4=BASE; SHADOW_ONLY; custo recalibração
- `evidence/m5-shadow-real.json` + `evidence/m5-shadow-real-output.txt` — shadow DDS real
- `evidence/m6-e2e-bloqueado.md` — bloqueio do E2E ativo e rota de destravamento
- `evidence/m7-regression.md` — comandos/totais + observação clippy pré-existente
- `../laya_t900_handoff/` está fora do repo: `src/laya_t900_handoff/` (raiz `src/`)

## Regras de honestidade aplicadas

T901–T903 permanecem intocados (`KEEP_T900`); pesos re-hashados pós-execução (preservação);
nenhum resultado antigo foi convertido em verificação atual; o que não rodou está marcado
NOT_RUN/BLOQUEADO, não "concluído".
