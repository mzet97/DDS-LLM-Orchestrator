# workers/laya-worker — worker Laya (fase 900)

Worker Python de decisão Laya (somente classificação; publicará `Routing.Decision`).
**Estado M1-02:** apenas os **bindings DDS gerados** (`dds_llm_workflow_v1/`) do
`crates/dds-contract/idl/WorkflowPlanning.idl` — SEM dependência Laya, sem CLI,
sem runtime (isso é T-900-M2-01+). Runtime alvo: `cyclonedds` 11.0.1 do PyPI
(classifiers 3.10–3.13; em 3.14 o binding importa, mas a série 11.0.1 não
declara suporte — pin do worker a definir em M2-01).

## Bindings gerados (T-900-M1-02)

- Fonte única: `../../crates/dds-contract/idl/WorkflowPlanning.idl`.
- Gerado por `idlc -l py` (backend idlpy 11.0.1). O backend **ignora `-o`** e
  escreve `./dds_llm_workflow_v1/` no CWD.
- Sem Laya: o pacote só depende de `cyclonedds` + stdlib.
- Regeneração: `regen_bindings.sh` (receita do backend `_idlpy` em
  `../../specs/900-laya-adaptive-routing/evidence/m1-01/README.md`).
- Hash pin (reprodutível — duas execuções geram bytes idênticos):
  - `_WorkflowPlanning.py` sha256 `19b0604f82b7678706c09a7bd5046ba51783931e53834bdfa6da33755e82741f`
  - `__init__.py` sha256 `9c6f11b973f2a669a91aa113608f019b1561f3a9235603781499ed67d9e30d60`

## Semântica de uso (emenda 3 do IDL)

Campos de enum são `int32` carregando o **ordinal** — passe
`m.QosIntent.QI_CRITICAL.value` (nunca o membro do enum: o pack falha).
`IdlEnum.auto()` aqui é 0-based, igual aos ordinais do IDL. Serialização:
`sample.serialize()` / `T.deserialize(bytes)` (métodos de instância/classe —
`IDL(T)` separado não popula a máquina da classe).

Verificado (M1-02): roundtrips CDR locais de `WorkflowIntent` (281 B),
`RoutingDecision` (417 B) e `WorkflowPlan` com `PlanNode` aninhado (349 B).
Interop de processos com Rust é T-900-M1-04 (V02/V03).
