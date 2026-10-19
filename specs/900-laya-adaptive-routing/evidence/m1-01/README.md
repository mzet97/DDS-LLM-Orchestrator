# Evidências T-900-M1-01 — contrato WorkflowPlanning isolado

Data: 2026-10-16/17 (UTC). HEAD `5a7c15b0be3ad0638d8a041338eda41c9587ae03`,
branch `studio/phase-800-node`. M0 preservado; nenhum commit criado.

## O que foi entregue

- `crates/dds-contract/idl/WorkflowPlanning.idl` — 5 tópicos do contrato M0-1
  (módulo `dds_llm_workflow_v1`), com duas emendas documentadas (prefixo de
  literais de enum; sem default de membro).
- `crates/dds-contract/idl/WorkflowPlanning.schema.json` — fixture congelada:
  ordem de campos, chaves, bounds, enums/ordinais, 5 tópicos/typenames.
- `crates/dds-contract/idl/WorkflowPlanning.digest_vectors.json` — 15 vetores
  golden do encoding de digest (payload_hash/plan_hash), com preimage hex.
- `crates/dds-contract/tests/workflow_planning_fixture.rs` — gate durável
  (3 testes, sem feature `dds`): IDL↔fixture, tópicos/chaves M0-1 e reprodução
  Rust dos vetores de digest (cross-language com a referência Python).
- Ferramenta de fixtures/checks: `../../tools/m1_01_fixture_tool.py`.

## Arquivos desta pasta

| Arquivo | Conteúdo |
|---|---|
| [generators.json](generators.json) | Versões idlc/idlpy/cyclonedds-build, comandos, exit codes, SHA-256 de todas as saídas e do IDL. |
| [probe-matrix.md](probe-matrix.md) | Matriz de capacidades/limitações dos 3 geradores, erros exatos e as 2 emendas + 1 risco aberto. |
| [generation-run.log](generation-run.log) | Execução capturada dos três geradores sobre o IDL final. |
| [hashes.txt](hashes.txt) | SHA-256 dos artefatos (inclui `WorkflowPlanning.c`, grande demais para copiar aqui). |
| [generated/](_WorkflowPlanning.py) | Saídas geradas: Python (`_WorkflowPlanning.py`, `__init__.py`), Rust (`dds_llm_workflow_v1.rs`), C header (`WorkflowPlanning.h`). |

## Verificações executadas

| Comando | Resultado |
|---|---|
| `idlc -l c -o out_c WorkflowPlanning.idl` (C 11.0.1) | exit 0, sem warning de extensibilidade |
| `idlc -l py WorkflowPlanning.idl` (idlpy 11.0.1, venv py3.13) | exit 0, gera `dds_llm_workflow_v1/` |
| scratch crate + `cyclonedds-build` 3.0.1 (`try_idlc=false`) | exit 0, gera `dds_llm_workflow_v1.rs` |
| `m1_01_fixture_tool.py check-py` / `check-rs` | ambos **OK** (ordem/chaves/bounds/enums vs fixture) |
| smoke Python runtime (venv py3.13): import + dataclass + chaves | 5 chaves exatas; typename qualificado |
| `cargo test -p dds-contract --locked --offline` | **18 passed** (8 suites), inclui 3 novos |
| `cargo clippy -p dds-contract --all-targets --locked --offline -- -D warnings` | exit 0 (warning restante é permissão de dir incremental no CIFS, não código) |
| `cargo fmt -p dds-contract --all -- --check` | exit 0 |

## Não verificado nesta tarefa (escopo M1-02..M1-04)

- Geração integrada no `build.rs` do dds-contract (T-900-M1-02).
- Geração Python em diretório do worker futuro + reproducibilidade/pin (M1-02).
- Wire XCDR/TypeIds reais entre processos Rust↔Python (V02/M1-04).
- QoS, late joiner, mismatch deliberado (V03).

## Reprodução do ambiente `_idlpy` (volátil em /tmp)

1. `pip download cyclonedds==11.0.1 --no-binary :all: --no-deps`
2. `python3.13 -m venv venv && venv/bin/pip install setuptools wheel`
3. `CYCLONEDDS_HOME=<sombra com include/, bin/, lib/ unificando lib+lib64 do ~/.local/cyclonedds> venv/bin/pip install --no-build-isolation --no-deps cyclonedds-11.0.1.tar.gz`
4. `PATH=<venv/bin>:~/.local/cyclonedds/bin:$PATH idlc -l py <idl>` (escreve em `./<modulo>/`, ignora `-o`).

Nada foi instalado no ambiente do usuário; tudo em /tmp (scratch).
