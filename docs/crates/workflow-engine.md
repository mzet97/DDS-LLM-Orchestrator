# `workflow-engine` — runtime adaptativo da fase 900 (Laya)

> Crate: `src/rust/crates/workflow-engine/` · Fase 900 (2026-10) · Estudo
> complementar (não-RQ5) · Worker Python: `src/rust/workers/laya-worker/` ·
> Dossiê SDD completo: `src/rust/specs/900-laya-adaptive-routing/`.

## Propósito

Runtime de orquestração **orientado a decisões de roteamento semântico**
(checkpoint Laya): uma `Workflow.Intent` é classificada pelo worker Laya em
uma `Routing.Decision`, validada, mapeada para QoS e materializada como um
plano DAG de tasks físicas determinísticas — tudo sobre DDS pub/sub
(tópicos do módulo `dds_llm_workflow_v1`), com journal SQLite transacional
para recuperação de crash.

## Peças

| Peça | Papel |
|---|---|
| `decision.rs` | Validator da `Routing.Decision` (`DecisionPolicy` com `calibration_pin`: `shadow_only=false` SÓ com pin sha256 completo casando) |
| `journal.rs` | SQLite WAL: reservas transacionais idempotentes, outbox atômico, IN_DOUBT persistido |
| `dag.rs` | Validador do plano candidato: ciclos, limites, orçamento agregado de tokens (profundidade memoizada desde o review PR#10) |
| `engine.rs` | Estado por nó (`NodeProgress` com `in_doubt` bloqueante), materialização idempotente por tentativa, joins por dependência |
| `runtime.rs` | Loop contra traits (`DataSpaceApi` + `WorkflowSpaceApi`): intents (CANCEL por `plan_revision`), decisões (shadow NÃO materializa), scheduler, timeouts (1×/tentativa → WS_IN_DOUBT) |
| `planner_http.rs` | `LlmHttpPlanner` (thread dedicada — `block_on` no reactor panica) |
| `qos_mapper.rs` | Objetivos semânticos → knobs (puro, ADR-002) |
| `eligibility.rs` | Anúncio `Agent.Capabilities` + catálogo rev 1 + janela de revogação |

Bins: **`wf-run`** (boot com `recover_from_journal`, `--calibration-pin`),
**`wf-observe`** (observador com pin), **`dag-validate`** (valida JSON de
plano candidato no stdin). ⚠️ O `client` também tem um bin `wf-run` legado
(workloads 880 `seq_chain_v1`…): construir ambos no mesmo target dir
sobrescreve um pelo outro — prefira `cargo run -p workflow-engine --bin wf-run`.

## Contrato (5 tópicos, módulo `dds_llm_workflow_v1`)

`Workflow.Intent`, `Routing.Decision`, `Agent.Capabilities`, `Workflow.Plan`,
`Workflow.State` — IDL `crates/dds-contract/idl/WorkflowPlanning.idl`
(enums como `long` ordinal; sem default de membro; fixtures golden com
digests reproduzidos Rust==Python).

## Task física determinística

`laya1-sha256(wf|plan_revision|node|attempt)` — mesma tentativa após crash
=restante mesmo task_id (recovery replanifica e republica o Plan, que a
barreira do agente exige); retry = NOVO task_id. O agente reivindica via
`--adaptive` (barreira Plan/State/Task; tarefas com barreira incompleta são
DEFERIDAS pelo sweeper, nunca descartadas).

## Honestidade documental (limites com evidência)

- **Calibração do Laya**: v4 REAL (SPROUT 12k/144k records, pin `10ef5945…`,
  score_mae −12% no teste real) mas `choice_accuracy` real = **0,148** — o
  checkpoint não classifica QoS em texto real (sintético media 0,55 =
  reconhecimento de template). Mais dataset de calibração NÃO resolve o
  bucket choice; noul/score transferem (ver `evidence/calibration/`).
- **Sem pin casando, decisões ficam em shadow** (nada materializado) — o
  caminho default do `wf-run` sem `--calibration-pin` é observação pura.
- **IN_DOUBT nunca re-executa automaticamente** (efeito incerto exige
  resolução externa); o recovery bloqueia nós nesse estado.
- O bug O-4b do orquestrador clássico (DONE dentro da janela de lease do
  writer morto) permanece ABERTO e afeta cenários failover deste runtime —
  ver `specs/905-otimizacao/o4b/`.
- Interop Rust↔Python provada em processos reais (V02/V03); E2E com
  inferência real 2/2 + 5 cenários (retry/restart/K2/IN_DOUBT/cancel) —
  ver `specs/900-laya-adaptive-routing/REPORT.md`.

## Ver também

`crates/agent.md` (modo `--adaptive`), `crates/dds-dataspace.md`
(`WorkflowSpaceApi`), Manual do worker: `workers/laya-worker/README.md`.
