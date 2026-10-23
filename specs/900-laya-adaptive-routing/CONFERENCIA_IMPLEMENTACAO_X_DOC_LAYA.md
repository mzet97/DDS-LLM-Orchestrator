# Conferência implementação × documentação — Laya (fase 900)

> Auditoria read-only executada em 2026-10-18 sobre o checkout
> `/var/mnt/HD1TB/tese/src/rust` (branch `studio/phase-800-node`, HEAD
> `fc56e60` ao auditar — revalide com `git -C src/rust rev-parse HEAD`).
> Objetivo: verificar se o que foi **implementado** está de acordo com o
> que está **documentado** (dossiê da fase 900 + handoff de implementação),
> antes da próxima etapa: **treinar o Laya**.
>
> **Veredito: CONFORME.** Todas as afirmações verificáveis bateram com
> código, hashes, artefatos brutos ou testes executados nesta auditoria.
> A única divergência encontrada é **documental** (dois textos superados),
> não de código nem de resultado experimental.

## 1. Evidência executada (comandos e resultados observados)

| # | Afirmação documentada | Como foi conferido | Resultado observado |
|---|---|---|---|
| 1 | Bindings Python gerados do IDL, pinados por hash | `sha256sum workers/laya-worker/dds_llm_workflow_v1/_WorkflowPlanning.py` e `__init__.py` | `19b0604f82b7678706c09a7bd5046ba51783931e53834bdfa6da33755e82741f` e `9c6f11b973f2a669a91aa113608f019b1561f3a9235603781499ed67d9e30d60` — **idênticos** aos pinos do README do worker |
| 2 | Contrato isolado de 5 tópicos; os 19 tópicos canônicos intactos | `crates/dds-contract/idl/WorkflowPlanning.idl` + fixtures ao lado; `topics::ALL` em `crates/dds-contract/src/lib.rs` | IDL, `WorkflowPlanning.schema.json` e `digest_vectors.json` presentes; `ALL: [&str; 19]` e teste que fixa `ALL.len() == 19` |
| 3 | Validator, catálogo, DAG, QoS mapper, journal e engine (`workflow-engine`) implementados | `cargo test -p dds-contract -p workflow-engine` | Todas as suítes `ok`, 0 falhas (validator de decisão, digest, fixtures sem feature `dds` incluídos) |
| 4 | Worker Python: adapter, `decide()` tipado, digest, perguntas V1 | `python3 -m pytest tests/ -q` em `workers/laya-worker/` | **25 passed, 20 subtests passed** |
| 5 | Barreira adaptativa do agente (`--adaptive`) e `WorkflowSpace` | `cargo test -p agent --lib adaptive`, `cargo test -p dds-dataspace --lib workflow` | Verdes (spot; ver §4) |
| 6 | M7: default NFCM virou estático | `crates/orchestrator/src/main.rs:48-51` | `#[arg(long, default_value = "static")]` com comentário "DEFAULT static desde T-900-M7-02" |
| 7 | CLI do worker: `--domain 211`, `--mock`, `--checkpoint-sha256`, `--calibration-path`, `--ttl-s` | `workers/laya-worker/src/laya_worker/cli.py:19-29` | Flags, defaults e gate R-LAYA (adapter real exige checkpoint) exatamente como documentado |
| 8 | Campanha M6: qualidade rules 0.906/0.9375, laya 0.75/0.8125 | `evidence/m6/campaign/QUALITY.json` (bruto) | `rules: qos_accuracy 0.9062, needs_planning_accuracy 0.9375`; `laya: 0.75 / 0.8125` (n=32) — confere |
| 9 | Campanha M6: validade planner-only 0.72 vs laya+planner 1.00 | idem | `planner_only.plan_validity 0.7188` (23 invocações); `laya_planner.plan_validity 1.0` (9 invocações) — confere |
| 10 | Campanha M6: latência P50 rules 0, laya 1,56 s, planner-only 35,2 s, laya+planner 22,4 s (−36%) | `evidence/m6/campaign/AGGREGATE.json` (bruto) | `p50_ms` = 0 / 1560 / 35156 / 22441; 30 runs, 120 amostras por braço; −36,2% calculado — confere |
| 11 | Checkpoint pinado e calibração v4 pinada | `evidence/calibration/calibration_report.json`, `tools/pilot_m6.py`, `tools/evaluate_real.py` | Checkpoint sha256 `891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c`; calibração pin `a235c9b90a2624ba11bb665e830556b03b1e1961d78cac5c1661895ad362e6b5` |
| 12 | Implementação real, não só árvore de trabalho | `git -C src/rust log --oneline -- crates/workflow-engine crates/dds-contract/idl/WorkflowPlanning.idl` | Commits `[T-900-M1]`, `[T-900-M2..M5]`, `[T-900-E2E-SCN+CAL-v2]`, `[T-900-A5+A6]`, `[PR10-review]` (correção de 7 P1 + 4 P2 de review) |

Hipóteses levantadas e **descartadas com evidência**: (a) "os números da
campanha foram inventados na doc" — conferem com os JSONs brutos;
(b) "refatorações quebraram o contrato" — regressão de 19 tópicos e
fixtures de digest verdes; (c) "código só existe não commitado" —
histórico Git mostra a implementação e uma rodada de review aplicada.

## 2. Divergências encontradas (documentais, não bloqueantes)

1. **`workers/laya-worker/README.md` está superado**: diz "Estado M1-02:
   apenas os bindings DDS gerados — SEM dependência Laya, sem CLI, sem
   runtime". O código atual tem CLI (`cli.py`), adapter real com gate
   por checkpoint, `decide.py`, `questions.py`, `space.py`, harness
   completo e 25 testes. Os **pinos de hash** desse README, porém,
   continuam corretos (item 1 da tabela).
2. **`PROMPT_CONTINUACAO_LAYA_DDS.md` é o handoff do M0**: afirma que
   a próxima IA "não está recebendo uma implementação Laya pronta".
   A fase 900 está completa (M1–M7). Não use esse arquivo como estado
   atual; use [HANDOFF_LAYA_IMPLEMENTACAO.md](HANDOFF_LAYA_IMPLEMENTACAO.md).

Nenhuma divergência de código, contrato ou resultado experimental foi
encontrada.

## 3. Para quem vai TREINAR o Laya (ler antes de começar)

Estado real do aprendizado hoje — não confunda com pipeline pronto:

- **O checkpoint pinado NÃO está calibrado.** O risco R-LAYA
  ([risks.md](risks.md)) registra: o checkpoint `891102d3…` tem
  **temperaturas inválidas** (confiança não calibrada). Por isso o uso
  em produção é **shadow** (decide, não materializa). Reproduzir o
  comportamento atual não exige retreinar; melhorar a decisão, sim.
- **O dataset seed não é calibration-grade.**
  `workers/laya-worker/dataset/dataset_v1_seed.jsonl` tem 32 exemplos
  PT/EN com **rótulos de engenharia** (não de modelo real) e splits
  fixos. Serve ao baseline de regras, **não** para treinar/calibrar.
- **ECE/Brier estão PENDENTES**: exigem modelo real + dados reais
  (registrado desde o M2). Thresholds de decisão **não congelados**:
  no split de teste do seed, baseline marcou qos 0.857 / planning 1.0 /
  macro-F1@0.5 0.552 — referência do baseline, não meta do modelo.
- **Material de calibração existente** em `evidence/calibration/`:
  `calibration_v1.json` … `calib_v4/` (pin atual `a235c9b9…`),
  `dataset_calib_v1.jsonl`, `dataset_snapshot400.jsonl`,
  `dataset_snapshot1050.jsonl`, `holdout.jsonl`, `sprout_poc_200.jsonl`,
  relatórios `calibration_report.json`/`calibration_report_v4.json`,
  `gates_v1.json` e `colab-limitacoes.md` (limitações do pipeline
  Colab). Ferramentas: `workers/laya-worker/tools/calibrate_laya.py`,
  `tools/colab_calib/`, `tools/colab_gen/`, `tools/gen_dataset.py`,
  `tools/merge_datasets.py`, `tools/sprout_to_laya.py`,
  `tools/evaluate.py` e `tools/evaluate_real.py`.
- **Baseline a bater** (campanha M6, LLM real gemma-4-12b UD-Q4_K_XL,
  4 braços × 30 runs — protocolo congelado em PROTOCOL_M6, manifests em
  `evidence/m6/campaign/`): qualidade laya atual 0.75/0.8125 contra
  rules 0.906/0.9375; o ganho comprovado do braço híbrido é
  **validade de plano 1.00 e −36% de latência P50** contra planner-only.
  Não alegue superioridade geral nem "vitória sobre MAF" (frente
  separada) a partir desses números.
- **Contrato de decisão é rígido**: `QUESTIONS_V1`
  (`src/laya_worker/questions.py`), probabilidades por perfil/
  capabilidade com `noul` preservado, score **sem arredondamento**,
  erros tipados `RC_*`, e tudo passa pelo **validator Rust**
  (`crates/workflow-engine/src/decision.rs`) — decisão do Laya é dado
  não confiável (D10) e é rejeitada com causa se violar schema,
  revisão esperada, `valid_until_ns` ou orçamento agregado. Treine
  para o contrato, não o contrário: mudanças de contrato exigem IDL +
  fixtures + regeneração dos dois lados (ver
  [HANDOFF_LAYA_IMPLEMENTACAO.md](HANDOFF_LAYA_IMPLEMENTACAO.md) §3).
- **Adapter real é gated**: `RealLayaAdapter` só carrega com
  `--checkpoint-sha256` conferido no load e import lazy
  (gate R-LAYA em `cli.py`/`adapter.py`). SDK: `laya==0.4.10`
  (Apache-2.0), torch CPU nesta máquina, venv dedicada.
- **Shadow antes de materializar**: `harness/run_shadow.py` e
  `run_calibrated_shadow.py` validam a decisão sem escrever Tasks
  (V04: `tasks=0`). Qualquer checkpoint novo deve passar por shadow +
  gates de calibração antes do braço híbrido (`wf-run`).

## 4. O que esta auditoria NÃO fez (incerteza declarada)

- Não reexecutou interop DDS ao vivo Rust↔Python, E2E com modelo real,
  sessão shadow nem a campanha M6 — a conformidade desses itens vem dos
  artefatos brutos preservados em `evidence/` e dos testes unitários/
  de contrato executados agora, não de reprodução completa.
- Os testes Rust de `agent`/`dds-dataspace` foram executados com filtro
  (`adaptive`, `workflow`), não as suítes inteiras dessas crates.
- Nenhum arquivo foi modificado por esta auditoria.

## 5. Ponteiros

- Estado e mapa da implementação: [HANDOFF_LAYA_IMPLEMENTACAO.md](HANDOFF_LAYA_IMPLEMENTACAO.md)
- Requisitos/invariantes: [spec.md](spec.md) · contratos: [contracts.md](contracts.md) ·
  arquitetura: [architecture.md](architecture.md) · ADRs 001–005: `adrs/` ·
  riscos vivos: [risks.md](risks.md) · ledger: [tasks.md](tasks.md) ·
  relatório por marco: [REPORT.md](REPORT.md) · evidências: `evidence/`
- Código: `crates/dds-contract/idl/WorkflowPlanning.idl`,
  `crates/workflow-engine/`, `crates/agent/src/adaptive.rs`,
  `workers/laya-worker/src/laya_worker/`, `workers/laya-worker/harness/`,
  `workers/laya-worker/tools/`
