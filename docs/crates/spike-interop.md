# `spike-interop` — spike de interop (não-produção)

> Crate: `src/rust/crates/spike-interop/` · **Harness de prova Fase 0b
> (REQ-101..105) — NÃO é código de produção.**

## Propósito

Validar interop Rust↔Python↔C++ via DDS: perfis espelham o `dds_backend` Python
(medição SEDP 2026-07-17); sem paridade (`Ownership=Exclusive` nos tópicos v4)
o matching reader↔writer não acontece. Entrega binários ponta-a-ponta de
validação + scripts Python/shell que fecham o loop (stubs Python, E2E A/B e
Rust-only).

## Binários (8, todos exigem `--features dds`)

| Binário | Uso |
|---|---|
| `pub-task` | `--count 10 --domain 0` — publica N `Task` PENDING e sai (sleep 2,5 s p/ discovery; `wait_for_acks` 10 s) |
| `sub-task` | `--domain 0 --timeout 30` — assina e valida campos |
| `llm-client` | `--domain 0 --timeout 60` — request/response LLM (exige llama-server `--enable-dds`) |
| `pub-stream` | `--count 1000 --domain 0` — Task dona + N chunks `TaskOutput` seq crescente |
| `sub-stream` | `--domain 0 --timeout 30` — conta gaps em `seq_num`; exit 1 se gap ou 0 chunks |
| `rtt-bench` | `--domain 0 --samples 10000 --warmup 100` — echo in-process + estatísticas; grava `benchmark_rust_results.json` no CWD |
| `repro-layout` | Domínio 60 fixo; A/B/C de layout `repr(C)`; `--skip-b` pula B (crash conhecido) |
| `diag-knobs` | Domínio 110 fixo, sem args; 4 `set_qos` manuais (delta zero, tprio, strength, latency) |

Extras: `dump-ops` (debug key serialization vs `idlc` C), bench criterion
`roundtrip` (domínio 62, sample 200, warmup 3 s), scripts `ab_coexistence.sh`
(T-207: 1 agente Rust + 1 Python disputam 100 tasks, cada task EXATAMENTE 1 vez)
e `e2e_rust_only.sh` (T-430: HTTP → orchestrator → agente → llama-server C++).

Lib: só `profiles::{tasks, task_output, llm}` atrás de `dds` (sem feature, vazia).
Sem structs/traits/erros próprios; bins usam `anyhow::Result`.

## Testes e limites

```bash
cargo test -p spike-interop --features dds   # só compila os bins — ZERO testes (nenhum #[test]/tests/)
CYCLONEDDS_STATIC=1 cargo bench -p spike-interop --features dds
```

**Não usar em produção**: QoS duplicada de `dds-dataspace` (drift futuro —
usar `dds-dataspace::qos::profiles`); bins com `unwrap`/`assert!` em dados de
rede, sleeps fixos, `println!` como protocolo, sem retry/shutdown gracioso;
domínios fixos colidem se paralelos; `rtt-bench` sobrescreve JSON no CWD e
trunca sub-ms (`mean as u64`); scripts com paths de máquina hardcoded
(`D=~/.cache/...`, `pkill -f`); `Task.priority` inconsistente entre bins (5×1);
parsing manual de args sem `--help`.
