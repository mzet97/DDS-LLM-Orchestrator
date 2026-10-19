# Debug Journal — estabilização Rust/CycloneDDS v1

Started: 2026-08-17T23:00:00-03:00
Goal: corrigir os defeitos confirmados da fase 500 e da auditoria de soundness, com testes e QA local.

## Environment snapshot (Phase 0)

- Runtime: rustc 1.95.0; cargo 1.95.0; stable ativo; nightly instalado.
- Entry: Cargo workspaces em `src/rust` e `third_party/cyclonedds-rust/cyclonedds-rust`.
- DDS: domínio de teste <= 232; build estático quando FFI for habilitada.
- Git HEAD (`src/rust`): `2059ab6996ce60b93b8610fe6848438c8f9a7266`.
- Working tree: sujo antes desta execução; arquivos pré-existentes listados no plano/relatório.
- Ferramentas: Docker disponível; `cmake` ausente no host; dev container será localizado antes dos gates FFI.
- References read: `debugging/runtimes/rust.md`, `00-setup.md`, `programming/rust/README.md`, `rust-ub/*`, `refactor/SKILL.md`.

## Hypotheses — T-701

1. [CONFIRMED] `SharedWaitSet::new` foi generalizado para `&impl DdsEntity`, mas `WaitSet::new` agora exige `&DomainParticipant`; evidência distintiva: E0308 e assinatura pública em `waitset.rs:119`; fix: restaurar tipo concreto.
2. [REFUTED] `WaitSet::new` possui overload genérico/por handle ainda utilizável; evidência: busca de definições encontra somente `&DomainParticipant`; fix alternativo não existe na API atual.
3. [REFUTED] A falha vem de feature/version skew no Cargo; evidência: o path dependency resolvido aponta para a mesma fonte local cuja assinatura foi lida; fix não é alteração de versão.

## Failed hypothesis round counter

- Round 1: hipótese 1 confirmada por compilador + assinatura; hipóteses 2/3 refutadas por inspeção do artefato resolvido.

## Artifacts to revert

- [ ] Este `.debug-journal.md`; remover no cleanup final.
- [ ] Entrada `.debug-journal.md` em `.git/info/exclude`; remover no cleanup final.
- [ ] `/tmp/codex-dds-rust-review-20260817/`; remover no cleanup final.

## Findings

- O PDF tem 157 páginas e descreve 18 tópicos canônicos, DDS-first local e gateway apenas para provedores externos.
- O runtime resolve `cyclonedds 3.0.0-alpha.1` por path local, divergindo da narrativa de versão publicada.
- Gate inicial do agente com `--features dds`: E0308 em `dispatch.rs:53`.

## Final fix

`DdsClientDds` passou a compartilhar dois pumps persistentes, um por tópico,
eliminando o fan-out quadrático. A confirmação do agente deixou de depender do
cache alimentado pelo stream sujeito a backpressure e passou a consultar o RHC
por handle de instância. O teste real com 60 submits passou em 33,88 s, com
`peak_registrations=2`, zero crescimento de threads e 60/60 conclusões.
### 2026-08-18 — closed-loop DDS benchmark
- Source: `crates/benchmarks/tests/dds_loopback.rs::driver_closed_loop_workers_concorrentes`
- Value: isolated run exits after 30.54 s with `submitted=4` (expected >=10).
- Interpretation: two workers each block near the 15 s request timeout; added temporary tracing initialization to expose claim/runtime state.
- Refutes/Confirms: refutes orphan-process-only cause and eager-reader-only cause.

### Green phase (2026-08-18)
- Fix: `crates/benchmarks/src/driver.rs` — cada worker closed-loop mantém um único stream de status durante toda a corrida; o background priority faz o mesmo.
- Test: `driver_closed_loop_workers_concorrentes` passou com pelo menos 24 conclusões observadas nos logs, contra `submitted=4` antes.
- Full suite: pendente de reexecução após esta correção.

### 2026-08-18 — 60 submits / SharedWaitSet
- Source: `crates/client/tests/client.rs::r2_shared_waitset_sob_client_submit_concorrente`
- Value: workspace serial permaneceu mais de 4 min em 919% CPU, sem concluir.
- Interpretation: gate de ausência de deadlock falha sob a configuração real de 60 submits; instrumentação temporária imprime pico de registros/threads e cada join concluído.
- Refutes/Confirms: investigação ativa.

### 2026-08-18 — fan-out observado
- Source: execução isolada com `--nocapture`.
- Value: `peak_registrations=120 peak_threads=40`; somente 5 `join_result=Ok(...)` antes de cessar progresso.
- Interpretation: 60 submits criam 120 readers e cada reader recebe o tráfego global dos tópicos, gerando trabalho quadrático e starvando o runtime.
- Refutes/Confirms: confirma H-fanout; correção é dois pumps persistentes + broadcast, não ampliar threads/timeouts.
