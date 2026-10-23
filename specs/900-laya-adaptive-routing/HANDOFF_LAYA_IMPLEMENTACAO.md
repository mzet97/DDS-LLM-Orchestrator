# Handoff — Implementação Laya (fase 900, roteamento adaptativo)

> **Para a próxima IA.** Escrito em 2026-10-18 a partir do código real.
> O arquivo [PROMPT_CONTINUACAO_LAYA_DDS.md](PROMPT_CONTINUACAO_LAYA_DDS.md)
> é o handoff do M0 (documental) e está **superado**: ele afirma que
> "você não está recebendo uma implementação Laya pronta" — não é mais
> verdade. **A fase 900 está COMPLETA (M1–M7).** Use este arquivo como
> ponto de entrada; o dossiê continua sendo a fonte de requisitos,
> decisões e evidências.

## 1. O que é

- **Laya** = modelo Python (pacote `laya==0.4.10`, Apache-2.0) que decide
  **objetivos semânticos de QoS/planejamento** por workflow. Só o Laya é
  Python; núcleo, planner/validator, engine, agentes e execução são Rust.
- Comunicação interna **só por CycloneDDS**, com um contrato IDL isolado
  de 5 tópicos (`WorkflowPlanning.idl`), separado dos 19 tópicos
  canônicos do restante do sistema (que não foram alterados).
- **Decisão do autor (2026-10-19): a frente FAZ PARTE da dissertação.**
  Trechos antigos do dossiê que dizem "estudo complementar/não-RQ5"
  estão superados; a forma editorial no `.tex` ainda será decidida.
- Semântica **at-least-once** (como o resto do projeto). Não alegar
  exactly-once, fencing fim-a-fim, DDS Security ou vitória sobre MAF.

## 2. Estado atual (verificado no código)

- Checkout: `/var/mnt/HD1TB/tese/src/rust` (raiz Git do workspace;
  a raiz `/var/mnt/HD1TB/tese` **não** é repo Git). Branch
  `studio/phase-800-node`, HEAD `fc56e60` ao escrever isto. Revalide
  com `git -C src/rust rev-parse HEAD` antes de confiar em números.
- Marcos: **M1–M5 concluídos, M6 integral executado** (campanha real
  4 braços × N=30 com LLM real + E2E do braço híbrido), **M7 concluído**
  (NFCM isolado; default estático). Ledger completo em
  [tasks.md](tasks.md); evidências em `evidence/m1-01` … `evidence/m6/`,
  `evidence/calibration/` e `evidence/m7-closing.md`.
- Checkpoint Laya pinado por sha256 (`891102d3…`, verificado no load) em
  venv dedicada. Calibração: `evidence/calibration/calib_v4`
  (pin `a235c9b9…`); números são **indicativos** (ver limitações).

## 3. Contrato (fonte única)

Arquivo: `crates/dds-contract/idl/WorkflowPlanning.idl`, módulo
`dds_llm_workflow_v1`. Fixtures congeladas ao lado:
`WorkflowPlanning.schema.json` e `WorkflowPlanning.digest_vectors.json`
(15 vetores golden Rust==Python). **Nunca** edite os tipos gerados na
mão; mude o IDL e regenere dos dois lados.

| Tópico | Tipo | Chave | Produtor → consumidor |
|---|---|---|---|
| `Workflow.Intent` | `WorkflowIntent` | `request_id` | cliente/ingresso Rust → intake + Laya |
| `Routing.Decision` | `RoutingDecision` | `request_id`, `decision_id` | Laya → validator Rust (shadow) |
| `Agent.Capabilities` | `AgentCapabilities` | `agent_id` | agente Rust → engine/agentes |
| `Workflow.Plan` | `WorkflowPlan` | `workflow_id`, `plan_revision` | engine Rust → agentes |
| `Workflow.State` | `WorkflowState` | `workflow_id`, `plan_revision` | engine Rust → agentes/cliente |

Emendas já congeladas ao contrato original (não "corrija"):
1. Literais de enum com prefixo (`IC_`, `QI_`, `DS_`, `RC_`, `NK_`,
   `WS_`, `NS_`) — escopo de módulo no IDL rejeita duplicatas.
2. Sem default `= 1` em membro (IDL 4.2 rejeita); `schema_version=1` é
   regra de validação do receptor.
3. Campos enum viajam como `long` com o **ordinal** (emenda 3): no
   Python, passe `m.QosIntent.QI_CRITICAL.value`, nunca o membro.
4. Risco conhecido: `cyclonedds-build 3.0.1` descarta o bound de
   `string<N>` no Rust (gera `DdsString`); C/Python preservam.

Bindings Python: gerados por `idlc -l py` em
`workers/laya-worker/dds_llm_workflow_v1/` (hashes pinados no README do
worker). Regenerar com `workers/laya-worker/regen_bindings.sh`. Serialização
Python: `sample.serialize()` / `T.deserialize(bytes)`.

## 4. Mapa da implementação

### Rust

| Crate / arquivo | Papel |
|---|---|
| `crates/dds-contract` (`workflow_mock.rs`, build.rs) | Tipos Rust do contrato, mocks e gate mock↔IDL sem feature `dds` |
| `crates/dds-dataspace/src/workflow_space.rs` | `WorkflowSpace` por composição: DDS real e in-memory, erros tipados (capacidade/versão/lag/conflito/orçamento), snapshot/restore |
| `crates/workflow-engine` | Domínio puro (sem DDS): `decision.rs` (validator — decisão Laya é dado **não confiável**, D10), `capability_catalog.rs` (catálogo rev 1), `eligibility.rs`, `dag.rs`, `qos_mapper.rs` (ADR-002: QoS de aplicação sobre endpoints fixos), `journal.rs` (SQLite/outbox/reservas), `engine.rs`/`runtime.rs` (readiness/recovery), `task_id.rs` (ID físico por tentativa). Bins: `wf-observe` (observador shadow), `wf_run`, `dag_validate` |
| `crates/agent/src/adaptive.rs` | Barreira adaptativa do agente: `AdaptiveBarrier`/`barrier_check` sobre Plan/State/Capabilities + target exato. Opt-in `--adaptive` (+ `--adaptive-capabilities`, `--adaptive-domain`); sem a flag, o claim legado é **intocado** |
| `crates/client/src/bin/wf_run.rs` | Driver de workload 100% Rust (braço híbrido Laya+planner do M6) |
| `crates/qos-nfcm` | **Legado preservado** (M7): default virou estático; loop NFCM condicional. Não remova a crate — orchestrator/benchmarks ainda usam tipos e o `StaticDecider` dela |

### Python — `workers/laya-worker/`

| Arquivo | Papel |
|---|---|
| `src/laya_worker/adapter.py` | Adapter Laya: `FakeLayaAdapter` determinístico (regras, offline) e adapter real gated por checkpoint (import lazy) |
| `src/laya_worker/decide.py` | `decide()` tipado: probabilidades por perfil/capabilidade, erros `RC_*` (timeout, modelo indisponível, expiração, duplicata) |
| `src/laya_worker/questions.py` | `QUESTIONS_V1` — perguntas versionadas do contrato de decisão |
| `src/laya_worker/rules_baseline.py` | Baseline de regras (braço de comparação das campanhas) |
| `src/laya_worker/digest.py`, `contract_schema.py` | Digest canônico e schema do contrato (espelham as fixtures) |
| `src/laya_worker/space.py` | Acesso DDS do worker (publica `Routing.Decision`, assina os demais) |
| `src/laya_worker/cli.py` | CLI DDS: `--domain` (default 211), `--mock`, `--checkpoint-sha256`, `--calibration-path`, `--ttl-s`, `--idle-timeout`, `--max-intents` |

Harness e ferramentas (não são runtime):
- `harness/run_shadow.py`, `run_calibrated_shadow.py` — sessão shadow
  (decide sem escrever Tasks; V04: `tasks=0`).
- `harness/run_interop.py`, `py_workflow_pub.py`, `py_workflow_sub.py` —
  interop Rust↔Python. Descoberta Python em loopback exige
  `harness/cyclonedds_loopback.xml` (`CYCLONEDDS_URI`).
- `harness/run_e2e.py`, `run_e2e_scenarios.py`, `flaky_proxy.py` — E2E.
- `tools/` — `campaign_m6.py`, `pilot_m6.py`, `llm_planner.py`,
  `evaluate.py`/`evaluate_real.py`, `gen_dataset.py`, `calibrate_laya.py`,
  `colab_calib/`/`colab_gen/` (calibração), `merge_datasets.py`.
- `dataset/dataset_v1_seed.jsonl` — seed PT/EN de **engenharia**,
  **não** calibration-grade (rótulos de engenharia; splits fixos).

## 5. Fluxo ponta a ponta

1. Cliente publica `Workflow.Intent` (SUBMIT/CANCEL, imutável).
2. Worker Laya classifica e publica `Routing.Decision` (perfis QoS +
   capabilities requeridas por limiar versionado; `noul` preservado
   como probabilidade; score sem arredondamento).
3. `workflow-engine` valida a decisão (schema, revisão esperada,
   `valid_until_ns`, orçamento agregado). Decisão inválida = rejeitada
   com causa tipada, nunca aplicada silenciosamente.
4. Engine materializa o `Workflow.Plan` (DAG validado), publica
   `Workflow.Plan`/`Workflow.State` e registra no journal SQLite.
5. Agentes com `--adaptive` anunciam `Agent.Capabilities` (heartbeat
   5 s, expira ≤10 s) e só reivindicam tasks do plano se a barreira
   aprovar (plano/estado/target exatos, elegibilidade vigente na janela
   do claim). Sem `--adaptive`, o comportamento é o legado.
6. QoS: a intenção semântica do Laya vira QoS de **aplicação** via
   `qos_mapper` sobre endpoints DDS estruturalmente fixos — nunca
   mutando writer compartilhado por requisição (ADR-002).

## 6. Resultados medidos (não extrapolar)

Campanha M6 (protocolo congelado antes dos runs, 4 braços × N=30,
LLM real gemma-4-12b UD-Q4_K_XL via prebuilt ROCm; manifests por run em
`evidence/m6/campaign/`, leitura sem viés no `campaign/README.md`):

- Qualidade (qos/planning): rules 0.906/0.9375 · laya 0.75/0.8125.
- Validade: planner-only 0.72 vs **laya+planner 1.00** (gate 9/32).
- Latência P50: rules ~0 · laya 1,56 s · planner-only 35,2 s ·
  **laya+planner 22,4 s** (−36% vs planner-only).

São resultados **desta** campanha e deste hardware/modelo. Não viram
tese de superioridade geral nem "vitória sobre MAF" (frente separada).

## 7. Como rodar (mínimo)

Pré-requisitos do workspace: ver `../AGENTS.md` (SDD, gates
`cargo test`/`clippy -D warnings`/`fmt --check`, sem `unwrap()` fora de
testes, arquivos ≤250 linhas úteis). Em CIFS/SMB use
`CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target` e
`CYCLONEDDS_STATIC=1` para builds com DDS real.

```bash
# Gates do contrato e do domínio (sem DDS)
cargo test -p dds-contract -p workflow-engine

# Worker shadow com adapter mock (offline, sem pesos)
cd workers/laya-worker
python -m laya_worker --mock --domain 211

# Interop Rust↔Python e shadow real: ver harness/ e a evidência do
# marco correspondente (evidence/m1-04, evidence/m2) antes de rodar.
```

## 8. Limitações e riscos vivos

- **Confiança do checkpoint não é calibrada**: temperaturas inválidas
  no checkpoint pinado → uso em **shadow**; o dataset seed não é
  calibration-grade; ECE/Brier seguem pendentes de modelo/dados reais
  (ver [risks.md](risks.md), R-LAYA).
- Domínio adaptativo é **isolado e opt-in**; experimentos legados
  (EXP/180/880) rodam idênticos sem `--adaptive`/`--adaptive-domain`.
- Interop Python exige a config de loopback do harness; sem ela a
  descoberta falha silenciosamente (sintoma clássico de "nada chega").
- R-PDF: o PDF exato da dissertação não foi localizado; validações
  acadêmicas usam a consolidação como fonte indireta.
- Arquivos gerados (`dds_llm_workflow_v1/`, `__pycache__/`) não se
  editam na mão; regenere pelos scripts.

## 9. Onde continuar

- Requisitos/invariantes: [spec.md](spec.md) (16 REQ-900-*) ·
  arquitetura: [architecture.md](architecture.md) · contratos:
  [contracts.md](contracts.md) · ADRs 001–005 em `adrs/` ·
  rastreabilidade: [traceability.md](traceability.md) · riscos:
  [risks.md](risks.md) · relatório por marco: [REPORT.md](REPORT.md).
- Regra da casa: antes de mexer, leia a task/REQ/ADR, escreva o teste
  que prova o requisito, implemente só aquilo, rode os gates, capture
  comando/saída como evidência e atualize tasks/traceability/REPORT.
  Erro ou bloqueio **não** é PASS — registre `BLOCKED` específico.
- Não crie commit, branch ou worktree sem autorização explícita da
  sessão (o histórico desta frente é de trabalho não commitado por
  padrão; confira `git status` antes de qualquer escrita).
