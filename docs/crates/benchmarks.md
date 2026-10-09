# `benchmarks` — carga E1–E5/OP1–OP4 (`dds-bench`)

> Crate: `src/rust/crates/benchmarks/` · Porte de `benchmarks/` e
> `benchmarks/qualificacao/` (Python) · Binário `dds-bench`.

## Propósito

Geração de carga E1–E5/OP1–OP4 + coleta JSONL no schema que a análise Python
consome. **A análise estatística (Friedman, mixed models, Jain, plots)
permanece no Python.**

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p benchmarks --features dds -- \
  --scenario E4 --domain 0 --duration 60 --arm nfcm --out ./bench_out
cargo run -p benchmarks --features dds -- --list   # lista os 9 cenários
```

Flags: `--scenario` (default E4), `--domain`, `--duration` (0⇒do cenário),
`--seed` (42), `--out` (`./bench_out`), `--model` (`local`), `--arm` (`nfcm`),
`--workers` (0⇒grade do cenário; N⇒1 nível ad-hoc), `--timeout-ms` (30000),
`--list`. Parse estrito: numérica inválida ou flag desconhecida = erro
(REQ/T-820-13). Sem `dds`: stub e sai. Env: `RUST_LOG`.

## Componentes

- **Cenários** (`scenarios.rs`): `WorkloadPattern` (Open/Closed/Priority/
  Streaming); `registry()` com 9 cenários E1–E5/OP1–OP4; `get()` case-insensitive;
  fontes Python citadas por cenário.
- **Regimes** (`regimes.rs`): LEVE λ=5, MODERADA λ=15, PESADA λ=30+bursts 50/0,5 s/10 s.
- **RNG** (`rng.rs`): xoshiro256** + SplitMix64; `f64`, `exponential`,
  `standard_normal`, `lognormal`. Paridade com NumPy é **estatística**.
- **Gerador** (`generator.rs`): `lambda_at` com burst, `next_inter_arrival`,
  `prompt_tokens` lognormal clamp [32,2048], `generate_prompt` cap 50 palavras.
- **Métricas** (`metrics.rs`): `RequestStatus` (ok/error/timeout/not_run);
  `RequestRecord` (ids, `test_id` minúsculo, `protocol="dds"`, `qos_arm`,
  `t_*_ns` E1, `ttfc_ms`/`icl_mean_ms`/`n_chunks` E5 com alias legado
  ttft/itl, `concurrency` aditivo); `JsonlWriter` (append bufferizado; `Drop`
  flusha com log).
- **Driver** (`driver.rs`, só `dds`): `BenchmarkDriver` — 1 participante DDS,
  `run_id` no nome do arquivo; `run()` (SIGINT drena in-flight);
  `submit_observed` (**captura a task terminal**: `assigned_agent`+`t_*_ns`;
  1 stream por loop, sem viés O(N²)); `one_request`; `one_stream` E5 (sem
  `is_final`→erro); `open_loop`; `closed_loop` grade por fase; `run_workers`;
  `priority_loop` (fundo `background_normal` + `injection_high`);
  `available_arms()` 9 braços; defaults: seed 42, `arm=nfcm`, timeout 30 s.

## Testes

```bash
cd src/rust
cargo test -p benchmarks
CYCLONEDDS_STATIC=1 cargo test -p benchmarks --features dds -- --test-threads=1
# tests/dds_loopback.rs (3: open E4, closed OP1, priority E3; domínio 103, agente MockEngine)
```

20 unitários (rng 5, generator 6, scenarios 6, metrics 3). Deps internas:
`client`, `dds-contract`, `dds-dataspace`, `qos-nfcm`; dev: `tempfile`, `agent`.

## Limites

- Sem análise: estatística e plots ficam no Python.
- `driver.rs` e `regimes.rs` sem unitários (só `dds_loopback` com `dds`).
- Closed-loop/priority usam prompt LEVE fixo — o regime não modula tamanho do
  prompt nesses padrões (só taxa/concorrência).
- OP3 externo ao driver: o kill do agente é operacional; o driver só mede a
  recuperação.
