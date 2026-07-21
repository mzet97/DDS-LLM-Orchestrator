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

**Measured performance (see [`PLANO_EXECUCAO.md`](./PLANO_EXECUCAO.md) for full methodology):**
- DDS state propagation: p50 **0.052ms** / p99 **0.077ms**
- Writer pool throughput: **88.7k tasks/s**
- 50 concurrent clients submitting tasks: zero deadlock
- End-to-end request (HTTP in → orchestrator → agent → LLM inference → result out): **458ms**

## How it works

```
Client → orchestrator (HTTP API) → DDS topics → agent → llama-server (C++ inference)
                                        ↑↓
              policy-engine · mcp-gateway · context-store · observability
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

## Project Structure

```
src/rust/
├── Cargo.toml                 # workspace manifest (14 members)
├── MIGRATION_PLAN.md          # design rationale and hardware target
├── PLANO_EXECUCAO.md          # detailed build/validation log with measured numbers
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

## Prerequisites

- Rust 1.85+ (`rustup show`)
- CMake 3.20+ (builds the CycloneDDS C library when the `dds` feature is enabled)
- The `cyclonedds` crate at `../../third_party/cyclonedds-rust` (path dependency, vendored)
- A cargo target dir **outside any SMB/CIFS mount** — the DDS C build fails
  `cmake_symlink_library` on CIFS:
  ```bash
  export CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target
  ```

## Quick Start

```bash
cd src/rust

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
see `src/llama_cpp/`):

```bash
CYCLONEDDS_STATIC=1 cargo run -p orchestrator --features dds
```

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

## Configuration

- DDS transport configs: `src/llama_cpp/dds/cyclonedds-*.xml`
- Set transport via: `CYCLONEDDS_URI=file://path/to/cyclonedds-*.xml`
- Build target dir (required off SMB/CIFS): `CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target`
- Static CycloneDDS link (required on SMB/CIFS): `CYCLONEDDS_STATIC=1`

## Documentation

- [`MIGRATION_PLAN.md`](./MIGRATION_PLAN.md) — design rationale and hardware target
- [`PLANO_EXECUCAO.md`](./PLANO_EXECUCAO.md) — detailed build/validation log with measured numbers per subsystem
- [`AGENTS.md`](./AGENTS.md) — conventions for whoever works in this crate
- [`specs/`](./specs/) — spec-driven-development docs (spec/plan/tasks/report) per subsystem
