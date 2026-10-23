# DDS-LLM Orchestrator (Rust)

A Rust workspace implementing a low-latency orchestration system for LLM
inference agents, using **DDS (Data Distribution Service)** as the data plane
instead of point-to-point HTTP.

## About

Instead of routing every request through HTTP hops, this system publishes
tasks, results, and telemetry onto DDS topics that clients, the orchestrator,
and inference agents all subscribe to. That gives the system:

- **Zero-copy, pub/sub delivery** — no per-hop HTTP/JSON overhead
- **Native QoS** — deadlines, liveliness, and ownership are enforced by the
  middleware, not application code
- **Decentralized discovery** — agents register and are selected without a
  central broker
- **High concurrency** — built on `tokio`/`rayon`, with no single-writer
  bottleneck on the hot path

A fuzzy/neuro-fuzzy QoS controller (`qos-nfcm`) continuously scores system
health and switches routing profiles (e.g. failover to a cheaper model) based
on live metrics, rather than static thresholds.

On top of that data plane, **phase 900+ adds Laya adaptive routing**: a Python
worker classifies each workflow intent (QoS profile, planning, complexity,
urgency, capabilities) and publishes a typed `Routing.Decision`; the Rust
`workflow-engine` validates it (pin de checkpoint/calibração, TTL, budget),
plans a DAG, journals it in SQLite and materializes `laya1-*` tasks. The Laya
model in production is the **T904 Unified checkpoint** (`bda679e7…`,
multi-teacher distilled; T900 `35813054…` kept as rollback) — see
[Adaptive routing (Laya)](#adaptive-routing-laya).

**Measured performance (see [`docs/history/PLANO_EXECUCAO.md`](./docs/history/PLANO_EXECUCAO.md) for full methodology):**
- DDS state propagation: p50 **0.052ms** / p99 **0.077ms**
- Writer pool throughput: **88.7k tasks/s**
- 50 concurrent clients submitting tasks: zero deadlock
- End-to-end request (HTTP in → orchestrator → agent → LLM inference → result out): **458ms**

## How it works

```
Client → orchestrator (HTTP API) → DDS topics → agent → llama-server (C++ inference)
                                        ↑↓
              policy-engine · mcp-gateway · context-store · observability

# adaptive path (opt-in, phase 900+)
Workflow.Intent → laya-worker (Python, Laya model) → Routing.Decision
    → workflow-engine validator → planner → DAG → Workflow.Plan/State
    → laya1-* Tasks → agent --adaptive → engine → result
```

1. A client submits a request over HTTP to the **orchestrator**.
2. The orchestrator publishes a `Task` on DDS, picking a QoS routing profile
   via `qos-nfcm`.
3. An **agent** claims the task (ownership arbitrated by DDS, so exactly one
   agent executes it), runs inference through `llama-server`, and streams
   results back as `TaskOutput` samples.
4. The orchestrator consolidates the stream and returns the HTTP response.
5. Tool calls, conversational context, and QoS/trace telemetry flow through
   dedicated topics handled by `mcp-gateway`, `context-store`, and
   `observability`.
6. **Adaptive path (opt-in)**: a `Workflow.Intent` is classified by the Laya
   worker into a `Routing.Decision` — the model's answer is treated as
   untrusted data until the Rust validator accepts it (shadow while the
   calibration pin doesn't match).

## Project Structure

```
.
├── Cargo.toml                 # workspace manifest (21 members)
├── AGENTS.md                  # conventions for contributors working in this crate
├── specs/                     # spec-driven-development docs (one folder per subsystem)
└── crates/
    ├── qos-nfcm/               # fuzzy/neuro-fuzzy QoS routing decisions
    ├── orch-common/            # shared types/metrics
    ├── dds-contract/           # IDL-generated DDS types
    ├── dds-dataspace/          # DDS pub/sub layer (topics, caches, writer pool)
    ├── agent/                  # task-claiming proxy in front of llama-server
    ├── orchestrator/           # control plane (HTTP API, scheduler, registry)
    ├── llm-gateway/            # LLM routing (pool, cache, rate-limit)
    ├── client/                 # task submission client
    ├── spike-interop/          # interop/benchmark harness
    ├── policy-engine/          # MCP tool-call policy engine
    ├── context-store/          # conversational context store
    ├── mcp-gateway/            # MCP tool gateway (filesystem/github/web)
    ├── observability/          # QoS/trace/metrics collectors
    └── benchmarks/             # load generator + workload driver
```

> **Migration note:** this is the Rust migration workspace. The legacy Python runtime lives in the main project under `src/orchestrator/`. The two runtimes share the same DDS topic namespace and IDL contract.

## Prerequisites

- Rust 1.85+ (`rustup show`)
- CMake 3.20+ (builds the CycloneDDS C library when the `dds` feature is enabled)
- The `cyclonedds` crate pinned from `https://github.com/mzet97/cyclonedds-rust.git` at the audited revision declared in `Cargo.lock`
- A cargo target dir **outside any SMB/CIFS mount** — the DDS C build fails
  `cmake_symlink_library` on CIFS:
  ```bash
  export CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target
  ```

## Quick Start

```bash
# Fast path — no DDS, mocks only
cargo check --workspace
cargo test --workspace

# Single crate
cargo test -p qos-nfcm

# Real DDS runtime (builds CycloneDDS C via cmake)
CYCLONEDDS_STATIC=1 cargo build -p agent --features dds
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1
```

`CYCLONEDDS_STATIC=1` links CycloneDDS statically, required when the workspace
lives on an SMB/CIFS mount (shared-library symlinks aren't supported there).
DDS-backed tests must run with `--test-threads=1` to avoid participant
contention.

Run the orchestrator end-to-end (requires a built `llama-server` with DDS —
see `src/llama_cpp/`; **build it STATIC: `-DBUILD_SHARED_LIBS=OFF` is mandatory
on 0.7.0**, the shared layout breaks DDS request delivery, verified 2026-10-21
in `notes.md` §I.10). Production binaries (2026-10-21): local RX 7900 XTX
`~/.cache/tese-benchmarks/llama-prebuilt/llama-070-hip/llama-server` (HIP,
Muse-Glimmer-30B + draft-dflash + vision), VM .61 `~/llama-070/llama-server`
(CUDA, gemma-4-12b + draft-mtp + vision on :8083, Qwen + DDS 170 on :8082),
VM .64 `~/llama-070/llama-server` (CPU). The historical fork rebuild with
gemma4+HIP lives in `third_party/llama.cpp_dds`, branch `sync/upstream-20261018`
(its O-1 parity patch is now merged into `src/llama_cpp`). Always start the
server with `--alias <model_name_used_by_tasks>`; the bridge silently drops
requests with a mismatched model. Use `-np 1` (`-np` auto forks the 0.7.0
server into router/child processes):

```bash
CYCLONEDDS_STATIC=1 cargo run -p orchestrator --features dds
```

One-shot task submission (benchmark/harness entry point; `ttfc_ms` in the
output = time to first chunk — 100% when not streaming):

```bash
cargo run -p client --features dds --bin submit-one -- 0 "model-name" \
  '[{"role":"user","content":"hi"}]' --timeout-ms 120000 --max-tokens 2048 --temperature 0.0 [--stream]
```

Recent optimizations (2026-10, evidence in `specs/905-otimizacao/`):
claim revalidation reads the arbitrated mesh (duplicate execution under burst:
0/700 measured; confirm window default 50 ms, override with
`AGENT_CONFIRM_DELAY_MS`), and DDS streaming with gemma4 delivers parity with
the HTTP engine byte-for-byte.

### Adaptive routing (Laya)

Fase 900 (decisão do autor: **faz parte da dissertação**) — um único modelo
Laya decide objetivos semânticos de QoS/planejamento; quem valida, planeja e
executa é o núcleo Rust. Semântica **at-least-once**; a decisão do modelo é
dado não confiável até o validator (`workflow-engine/src/decision.rs`).

**Contrato**: 19 tópicos DDS canônicos intactos + 5 tópicos opt-in
(`WorkflowPlanning.idl`): `Workflow.Intent`, `Routing.Decision`,
`Agent.Capabilities`, `Workflow.Plan`, `Workflow.State`.

**Checkpoint vigente (T904 Unified, promovido pelo autor em 2026-10-22)**:

| | |
|---|---|
| Pesos | `bda679e71295593904370eba5d9130609aea985f0b0d183d2527fcd398ca039a` |
| Calibração | pin `6e1543f3…` (split de calibração dedicado) |
| Caminho de produção | `~/.cache/tese-laya/checkpoints/laya-production` |
| Rollback | T900 `35813054…` (`~/.cache/tese-laya/checkpoints/teachers/t900-specialized`) |
| Sealed T904 (n=567, OOD real) | QoS F1 **0.823** (T900: 0.492) · STREAM P/R/F1 **0.936/0.903/0.919** (T900: 0.214) · planning 0.926 (T900: 0.302) |

Gates absolutos de urgency/planning/complexity/caps ficaram registrados como
limitações conhecidas (alvos herdados de holdouts sintéticos; recalibração
planejada no [T905](./specs/905-laya-ood-recalibration/PLAN.md)).

**Executar o worker** (requer a venv do SDK — `laya==0.4.10` em
`~/.cache/tese-laya/venv`; checkpoint completo montado por
`workers/laya-worker/tools/assemble_checkpoint.py`, já disponível em
`laya-production`):

```bash
CYCLONEDDS_URI=file://workers/laya-worker/harness/cyclonedds_loopback.xml \
PYTHONPATH=workers/laya-worker/src ~/.cache/tese-laya/venv/bin/python -m laya_worker \
  --domain 231 \
  --checkpoint-path ~/.cache/tese-laya/checkpoints/laya-production \
  --checkpoint-sha256 bda679e71295593904370eba5d9130609aea985f0b0d183d2527fcd398ca039a
```

O worker assina `Workflow.Intent` e publica **somente** `Routing.Decision`
(sem caminho para `Tasks`); decisões com calibração vazia/divergente ficam em
`shadow_only` no validator. Shadow observável com
`workers/laya-worker/harness/run_shadow_t904.py` (gate: `tasks_seen=0`).

**Rollback** (uma linha): trocar `--checkpoint-path`/`--checkpoint-sha256`
para o T900. Histórico científico completo: `specs/904-laya-unified-multiteacher/`
(campanha, ablações, sealed único, decisão) e `specs/904-laya-runtime-integration/`.

### DDS Security (local-only)

The runtime supports OMG DDS Security via the `security` feature. This is
**intentionally local-only for now** — it authenticates participants with
X.509 certificates and encrypts discovery/user data, but it does not replace
network-level segmentation and is not exposed as a multi-host deployment option.

```bash
# Run the security smoke test (uses separate processes because CycloneDDS
# skips the security handshake for in-process participants).
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds,security \
  --test security_smoke -- --test-threads=1

# Run a component with DDS Security enabled (uses certificates under
# config/dds/security/ by default).
CYCLONEDDS_STATIC=1 cargo run -p orchestrator --features dds,security -- \
  --dds-secure --dds-security-dir ./config/dds/security
```

See `config/dds/security/generate-certs.sh` for regenerating the local test
artefacts (identity/permissions CAs, governance and permissions P7S). These
certificates are test fixtures only; do not reuse them in production.

## Crates

| Crate | Role |
|---|---|
| `qos-nfcm` | Neuro-Fuzzy Cognitive Map QoS controller — 5 selectable decision strategies (static/zadeh/fcm/fcm-dhl/nfcm) |
| `orch-common` | Shared types and metrics used across the workspace |
| `dds-contract` | DDS types generated from IDL, plus QoS profiles |
| `dds-dataspace` | DDS pub/sub layer: topics, sharded caches, writer pool, QoS monitor |
| `agent` | Claims tasks off DDS, runs inference via `llama-server`, streams results back |
| `orchestrator` | HTTP API, scheduler, agent registry, QoS control loop |
| `llm-gateway` | Routes inference requests across local/cloud providers with cache and rate-limiting |
| `client` | Submits tasks and consumes streamed results |
| `spike-interop` | Standalone interop/benchmark harness for the DDS wire format |
| `policy-engine` | Evaluates tool-call policies published as DDS snapshots |
| `context-store` | Ingests and serves conversational context over DDS |
| `mcp-gateway` | Routes MCP tool calls (filesystem/github/web) with policy enforcement |
| `observability` | Collects QoS metrics, violations, and execution traces |
| `benchmarks` | Generates load (Poisson/burst traffic) and drives validation scenarios |
| `workflow-engine` | Fase 900 (Laya adaptive routing): validator de `Routing.Decision` com pin de calibração, journal SQLite transacional, DAG validator, planner HTTP — ver `specs/900-laya-adaptive-routing/` |
| `studio-*` (`orchestrator-studio`, `studio-core`, `studio-node`) | Studio: GUI egui multi-host (19º tópico `Studio.NodePresence`) + daemon de instalações |

Extras fora de `crates/`: `workers/laya-worker/` (worker Python Laya real + harness de
calibração/interop/E2E/shadow + ferramentas de dataset/treino da T904) e
`specs/905-otimizacao/` (evidências das otimizações O-1/O-2/O-4b do plano de otimização
do orquestrador).

## Configuration

- DDS transport configs live in the main project at `src/llama_cpp/dds/cyclonedds-*.xml` and `config/dds/`; this Rust workspace consumes the same topic namespace but does not ship the C++ server configs
- Set transport via: `CYCLONEDDS_URI=file://path/to/cyclonedds-*.xml`
- **Todos os processos de um mesmo teste DEVEM usar a mesma `CYCLONEDDS_URI`** (config divergente entre processos = zero descoberta mútua; o default do host mira uma NIC stale — lição O-1, ver `specs/905-otimizacao/o1/README.md`)
- Build target dir (required off SMB/CIFS): `CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target`
- Static CycloneDDS link (required on SMB/CIFS): `CYCLONEDDS_STATIC=1`

## Documentation

- [`docs/history/MIGRATION_PLAN.md`](./docs/history/MIGRATION_PLAN.md) — design rationale and hardware target
- [`docs/history/PLANO_EXECUCAO.md`](./docs/history/PLANO_EXECUCAO.md) — detailed build/validation log with measured numbers per subsystem
- [`AGENTS.md`](./AGENTS.md) — conventions for whoever works in this crate
- [`specs/`](./specs/) — spec-driven-development docs (spec/plan/tasks/report) per subsystem
- [`specs/904-laya-unified-multiteacher/`](./specs/904-laya-unified-multiteacher/) — campanha T904 Unified (treino, sealed, promoção) + evidências em `artifacts/`
- [`specs/904-laya-runtime-integration/`](./specs/904-laya-runtime-integration/) — integração do runtime com checkpoint local explícito
- [`specs/905-laya-ood-recalibration/PLAN.md`](./specs/905-laya-ood-recalibration/PLAN.md) — plano T905 (recalibração OOD dos gates)
- [`docs/`](docs/) — official documentation (PT-BR): hub + one page per crate + Studio user manual
