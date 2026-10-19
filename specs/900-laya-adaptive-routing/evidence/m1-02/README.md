# Evidências T-900-M1-02 — integração do contrato no dds-contract + bindings Python

Data: 2026-10-17 (UTC). HEAD base `5a7c15b0` (sem commit, por autorização de sessão não incluir commits).

## Entregas

| Arquivo | Papel |
|---|---|
| `crates/dds-contract/build.rs` | Compila `WorkflowPlanning.idl` (parser interno, `try_idlc=false`), qualifica typenames e injeta blobs XTypes do `idl/WorkflowPlanning.c` (idlc C 11.0.1, agora checked-in como os `.c` legados) |
| `crates/dds-contract/idl/WorkflowPlanning.c` | Saída idlc C 11.0.1 (sha256 `d17c2b6e86059a4bf08e487a38e544c132fbb36c3334f6af52e3165ccea3702d`) |
| `crates/dds-contract/src/lib.rs` | `workflow_topics` (5 consts, isolados de `topics::ALL[19]`), `typenames::*` novos 5, `workflow::dds_llm_workflow_v1` (dds=gerado / não-dds=mock) |
| `crates/dds-contract/src/workflow_mock.rs` | Mock gerado por `tools/m1_02_mock_gen.py` a partir da fixture (enums i32 — emenda 3) |
| `crates/dds-contract/tests/workflow_planning_wire.rs` | 8 testes com `--features dds`: typenames, chaves, isolamento dos 19, roundtrips XCDR1/2 (intent/plan com sequences aninhadas/decision), rejeição de bound excedido, blobs de metadados |
| `crates/dds-contract/tests/workflow_planning_fixture.rs` | +1 teste `not(dds)`: paridade mock↔fixture (campos/ordem via serde) + ordinais dos enums |
| `workers/laya-worker/` | Bindings Python gerados (`dds_llm_workflow_v1/`) + README + `regen_bindings.sh` com hash pin; sem dependência Laya |

## EMENDA 3 ao contrato (reportada antes de congelar)

Campos de enum nas structs viram `long` carregando ordinal: cyclonedds 3.0.1
não implementa `DdsType` para enums (`DdsEnumDerive`), então structs de tópico
com campo enum-tipado NÃO COMPILAM. Precedente V4 (`Task.status`,
`Task.model_required`). Os 15 vetores golden de digest permaneceram com hashes
**idênticos** após a emenda (u32 BE ordinal ≡ i32 BE para valores válidos) —
regeneração e verificação executadas (`SCHEMA CHECK OK`, `DIGEST CHECK OK`).
No Python gerado, campos são `int32`: passar `Enum.MEMBER.value` (o membro do
enum faz o pack falhar; `IdlEnum.auto()` é 0-based = ordinais IDL, conferido).

## Verificações executadas (2026-10-17)

| Comando | Resultado |
|---|---|
| `idlc -l c` (regeneração do .c pós-emenda 3) | exit 0, sem warning de extensibilidade |
| `idlc -l py` ×2 (bindings worker) | bytes idênticos nas duas execuções (hashes pinados no README/regen script) |
| roundtrips CDR Python (wheel 11.0.0.1 py3.14, instância `serialize()`/`deserialize()`) | intent 281 B, decision 417 B, plan com PlanNode aninhado 349 B — todos OK |
| `cargo test -p dds-contract --locked --offline` | **19 passed / 0 failed** (9 suites) |
| `cargo test -p dds-contract --features dds --locked --offline` | **48 passed / 0 failed** (inclui regressão dos 19 tópicos: lib dds_tests, contract_v4, interop, t808) |
| `cargo clippy -p dds-contract --all-targets [--features dds] -- -D warnings` | exit 0 (ambos) |
| `cargo fmt -p dds-contract --all -- --check` | exit 0 |

## Não verificado aqui

- Interop de PROCESSOS Rust↔Python em DDS real (V02/V03) — T-900-M1-04.
- Runtime DDS publicando os 5 tópicos (WorkflowSpace) — T-900-M1-03.
- O wheel 11.0.1 declara classifiers 3.10–3.13; o py3.14 do host importa e
  serializa, mas o pin oficial do worker fica para M2-01.

## Rollback

Reverter build.rs/lib.rs/workflow_mock.rs/testes + remover
`idl/WorkflowPlanning.c`, `workers/laya-worker/` e dev-dep sha2. IDLs antigos
e os 19 tópicos permanecem sem diff (verificado por git).
