# Documentação oficial dos projetos Rust

> Workspace: `src/rust/` (resolver 2, edition 2021, Rust ≥ 1.85, `warnings = deny`).
> Idioma desta documentação: português (PT-BR). O `README.md` do workspace (em inglês)
> continua sendo a referência rápida de build.

Esta é a documentação oficial dos 18 crates Rust do DDS-LLM-Orchestrator —
runtime DDS-first, serviços, benchmark e o desktop Studio — mais o
**[Manual do Usuário do Studio](studio-manual.md)**.

## Mapa dos crates

| Crate | Papel | Doc |
|---|---|---|
| `orch-common` | Tipos compartilhados, enums do wire, instrumentação | [crates/orch-common.md](crates/orch-common.md) |
| `dds-contract` | Contrato DDS único: tipos IDL, tópicos, QoS | [crates/dds-contract.md](crates/dds-contract.md) |
| `dds-dataspace` | Camada DDS: DataSpace, caches, streams, pools | [crates/dds-dataspace.md](crates/dds-dataspace.md) |
| `orchestrator` | Control-plane: HTTP, scheduler, reapers, QoS | [crates/orchestrator.md](crates/orchestrator.md) |
| `agent` | Worker: claim PENDING, engines dds/http/mock | [crates/agent.md](crates/agent.md) |
| `llm-gateway` | Roteamento LLM: cache, rate-limit, failover | [crates/llm-gateway.md](crates/llm-gateway.md) |
| `client` | Cliente DDS + drivers de workload/benchmark | [crates/client.md](crates/client.md) |
| `context-store` | Persistência de contexto conversacional | [crates/context-store.md](crates/context-store.md) |
| `policy-engine` | Fonte da verdade das políticas de segurança | [crates/policy-engine.md](crates/policy-engine.md) |
| `mcp-gateway` | Fronteira MCP via `ToolCall.Request` | [crates/mcp-gateway.md](crates/mcp-gateway.md) |
| `observability` | Eventos, QoS store, traces, trackers | [crates/observability.md](crates/observability.md) |
| `qos-nfcm` | Decisores QoS (NFCM/fuzzy/bandits) — legado de disciplina | [crates/qos-nfcm.md](crates/qos-nfcm.md) |
| `benchmarks` | Carga E1–E5/OP1–OP4 (`dds-bench`) | [crates/benchmarks.md](crates/benchmarks.md) |
| `spike-interop` | Spike de interop Rust↔Python↔C++ (não-produção) | [crates/spike-interop.md](crates/spike-interop.md) |
| `det-responder` | Respondedor determinístico p/ benchmark (não-produção) | [crates/det-responder.md](crates/det-responder.md) |
| `orchestrator-studio` | Desktop GUI (egui) — ver manual | [studio-manual.md](studio-manual.md) + [crates/orchestrator-studio.md](crates/orchestrator-studio.md) |
| `studio-core` | Domínio puro do catálogo (revisões, OCC) | [crates/studio-core.md](crates/studio-core.md) |
| `studio-node` | Nó administrativo HTTP + presença DDS | [crates/studio-node.md](crates/studio-node.md) |

Grafo de dependência (alto nível):

```
dds-contract ──┬──▶ dds-dataspace ──┬──▶ orchestrator ──▶ studio (via DDS/HTTP)
orch-common ───┘                    ├──▶ agent ──▶ llm-gateway
                                    ├──▶ client ──▶ benchmarks
                                    ├──▶ context-store / policy-engine ──▶ mcp-gateway
                                    ├──▶ observability
                                    └──▶ studio-node ──▶ orchestrator-studio (+ studio-core)
qos-nfcm ──▶ orchestrator, benchmarks        spike-interop, det-responder: harness isolados
```

## Convenções do workspace

- **Features**: `default = []` (HTTP/mock, sem DDS) em quase todos; `dds` liga o
  CycloneDDS real; alguns têm `security` (DDS Security, opt-in).
- **Build DDS real** (especialmente em SMB/CIFS): prefixar `CYCLONEDDS_STATIC=1`
  (link estático). Ex.: `CYCLONEDDS_STATIC=1 cargo test -p orchestrator --features dds`.
- **Testes DDS**: serializar com `-- --test-threads=1`; cada arquivo usa domínios
  DDS fixos e isolados.
- **Gates**: `cargo clippy -p <crate> --all-targets -- -D warnings` e
  `cargo fmt --check` (lints `warnings = deny` no workspace).
- **Versões internas**: path-deps pinadas em `=0.1.0`; `cyclonedds =3.0.1`
  (+ `-sys =1.2.1`, `-build =3.0.1`).
- **Exceção MSRV dev-only**: produção compila com rustc 1.85, mas
  `cargo test -p orchestrator-studio` exige toolchain ≥ 1.95 (dependência
  de teste `egui_kittest`).

## Contrato DDS (resumo)

19 tópicos canônicos (`Tasks`, `AgentRegistry`, `TaskOutput`, `SystemMetrics`,
`LLM.InferenceRequest/Result/Error`, `ServerStatus`, `Context.Snapshot/Update`,
`ToolCall.Request`, `Security.PolicySnapshot/Update`, `Execution.Trace`,
`QoS.RoutingProfile/Metric/Violation/Discovery`, `Studio.NodePresence`).
Detalhes, typenames, chaves e perfis QoS: [crates/dds-contract.md](crates/dds-contract.md).

## Fluxo operacional mínimo

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p orchestrator --features dds -- --dds-domain 0
CYCLONEDDS_STATIC=1 cargo run -p agent --features dds -- --agent-id agent-01 --dds-domain 0
cargo run -p studio-node --bin studio-noded            # 127.0.0.1:4317
cargo run -p orchestrator-studio --bin studio --features dds   # GUI (ver manual)
```

## Honestidade documental

Cada página lista seus **limites e gaps** com evidência no código. Regras gerais:
não afirmar exactly-once (é at-least-once), fencing fim-a-fim, MCP ativo além
do filesystem, DDS Security ativo por padrão, ou vitória sobre baselines —
a instrumentação e as campanhas estão documentadas em seus crates.
