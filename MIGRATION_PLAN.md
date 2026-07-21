# Plano de Migração para Rust — Orquestrador DDS-LLM

Migração incremental dos projetos de `tese/src/*` (Python) para Rust, usando a
crate local **`cyclonedds`** (`third_party/cyclonedds-rust`, autor Matheus Zeitune).
O `llama_cpp` **permanece em C++** (é o motor de inferência, não a coordenação).

> **Princípio.** Nada de *big-bang*. Todos os nós — Python, C++ e Rust — falam o
> **mesmo wire format DDS (XTypes/XCDR)** gerado do **mesmo `OrchestratorDDS.idl`**.
> Logo, cada componente migra sozinho, roda **lado a lado** com o Python e é
> comparável em **A/B** nos mesmos tópicos.

## Hardware alvo (informa as escolhas)
- **Ryzen 9 5900X — 12c/24t** → runtime `tokio` multicore + `rayon` data-parallel.
  O treino do NFCM, serial no Python (GIL), vira paralelo nos 24 threads.
- **64 GB RAM** → caches concorrentes (`dashmap`) e loans zero-copy sem pressão.
- **RX 7900 XTX 24 GB (ROCm)** → fica com o `llama_cpp` (C++). Opcional futuro:
  treino do NFCM em GPU via `candle`/`burn` com backend ROCm (não necessário agora).

---

## 1. Estrutura do workspace (`tese/src/rust/`)

```
src/rust/
├── Cargo.toml                 # workspace (perfil release: LTO, codegen-units=1, panic=abort)
├── rust-toolchain.toml        # stable (>=1.85, exigido pela crate cyclonedds)
├── crates/
│   ├── orch-common/           # tipos/config/métricas (substitui common/)   [compila ✓]
│   ├── qos-nfcm/              # NFCM completo — IMPLEMENTADO + testado        [7 testes ✓]
│   ├── dds-contract/         # tipos do OrchestratorDDS.idl + perfis QoS     [scaffold]
│   ├── dds-dataspace/        # camada DDS: WaitSet, caches lock-free, zero-copy [scaffold]
│   ├── llm-gateway/          # roteamento a provedores, multi-worker real     [scaffold]
│   ├── agent/                # bin: assume tasks + ponte llama-server C++      [scaffold]
│   ├── orchestrator/         # bin: axum + scheduler + decisor NFCM           [scaffold]
│   └── client/               # submete tasks (resolve deadlock de 20 clientes) [scaffold]
```

`cargo check --workspace` compila as 8 crates **hoje** (a dependência `cyclonedds`
é opcional, feature `dds`, para não disparar o build C do CycloneDDS na verificação).

---

## 2. Mapa componente Python → crate Rust

| Origem (`src/orchestrator/…`) | LOC~ | Crate Rust | Técnica de perf | Fase |
|---|---:|---|---|:--:|
| `agent/` | 2,0k | `agent` (bin) | tokio multi-task; escrita de streaming sem thread única; ponte DDS ao C++ | **1** |
| `dds_backend/` | 3,4k | `dds-dataspace` | WaitSet+`take_aiter`; zero-copy loans; `dashmap`; pool de writers MPMC | **2** |
| `orchestrator/` | 2,0k | `orchestrator` (bin) | axum/hyper (ingressão); scheduler lock-free; loop de controle async | **3** |
| `client/` | 0,2k | `client` | 1 participante servindo N tasks (resolve deadlock ≥20) | **3** |
| `llm_gateway/` | 1,0k | `llm-gateway` | worker pool real (`Semaphore`), sem GIL corrompendo métricas | **3** |
| `dds_types.py` | — | `dds-contract` | gerado do IDL via `cyclonedds-idlc` → mata o *drift* Py↔C++ | **0** |
| `neuro_fuzzy/` | 1,0k | `qos-nfcm` ✅ | inferência sem alocação; **treino paralelo (rayon)** | **feito** |
| `common/` | 0,3k | `orch-common` ✅ | métricas atômicas (some o bug C3); `tracing` JSON | **feito** (base) |
| `fuzzy_qos_manager/`, `fcm_qos_manager/` | 1,2k | (portar p/ `qos-nfcm` como baselines) | — | 4 |
| `llama_cpp/` (C++) | — | **mantém C++** | já nativo | — |
| `automation/` (ansible) | — | fora de escopo | — | — |

### Subsistemas adicionais (revelados pela arquitetura da dissertação — ver `specs/DISSERTACAO.md`)
A visão do autor (figuras `F23`/`F31`/`F32` verificadas) inclui componentes além do que o
`src/orchestrator/` explorado mostrava. Entram como crates/fases adicionais:
| Nova crate | Substitui | Fase |
|---|---|---|
| `policy-engine` | motor de políticas (YAML → snapshot DDS `SecurityPolicy`; caches locais) | 3 |
| `mcp-gateway` | gateway de ferramentas (MCP + governança pela política distribuída) | 3–4 |
| `context-store` | persistência de contexto conversacional (DDS → PostgreSQL) | 4 |
| `observability` | coletores QoS/Trace/Metrics (DDS → PostgreSQL; 12 condições do CycloneDDS) | 4 |
| `compat-http` / `compat-grpc` | backends de comparação (adapters da abstração de transporte) | 4 (opcional) |
| `benchmarks` | geração de carga E1–E5 / OP1–OP4 | contínuo |

**Padrão arquitetural a preservar (F31):** comunicação **só por interfaces** (`ITransport`,
`IPublisher<T>`, `ISubscriber<T>`, `IParticipant`; domínio `ITask`/`IAgentState`/`IInference`/
`IToolCall`/`ISecurityPolicy`/`IQoSEvent`). Isso valida a `trait DataSpaceApi` da `dds-dataspace`
e mantém os backends DDS/HTTP/gRPC intercambiáveis. Ciclo de vida da tarefa (F25):
`CREATED→PENDING→CLAIMED→RUNNING→COMPLETED` + recuperação (`RECOVERY_PENDING`).

---

## 3. Fases

### Fase 0 — Contrato + spike de interop *(1–2 semanas)*
- `cyclonedds-idlc --input src/llama_cpp/dds/idl/OrchestratorDDS.idl --output-dir crates/dds-contract/src/generated/` → tipos Rust wire-compatible.
- Nó Rust mínimo publica/assina `Tasks`/`LLM.*` e **interopera com o orquestrador Python + o llama-server C++** nos mesmos tópicos.
- **Benchmark** round-trip Rust-vs-Python (o harness `cyclonedds-bench` já existe: `latency`, `throughput`, `ipc`). **Quantifica o ganho antes de comprometer.**

### Fase 1 — Agente *(maior ROI)*
Migrar `agent/` para o binário `agent`. Rodar **um agente Rust** ao lado dos agentes
Python e comparar latência/throughput/CPU sob a mesma carga. Ganho: caminho de
streaming sem serialização + sem GIL.

### Fase 2 — Camada DDS (`dds-dataspace`)
Onde o GIL mais dói. WaitSet nativo + zero-copy + `dashmap` + pool de writers.
Remove o poll loop, o churn por amostra e a thread única de escrita de uma vez.

### Fase 3 — Control plane
`orchestrator` (axum + scheduler/registry/selector), `client` (fim do deadlock de
20) e `llm-gateway` (multi-worker real). O decisor **NFCM já está pronto** (`qos-nfcm`).

### Fase 4 — Baselines & consolidação
Portar Zadeh/FCM/DHL para dentro de `qos-nfcm` como baselines (comparação do artigo)
e desligar os componentes Python equivalentes.

---

## 4. Como Rust remove cada gargalo (do relatório de investigação)

| Gargalo Python (evidência no código) | Solução Rust |
|---|---|
| **GIL** — tudo serializa num lock | Sem GIL: readers/writers/workers/treino paralelos nos 24 threads |
| **Deadlock ≥20 clientes** (20 participantes × GIL) | 1 participante servindo N tasks async (`client`) |
| **Thread única de escrita** | Pool de writers + `crossbeam-channel` MPMC |
| **Churn de objetos por amostra** (`dds_to_task`) | **Zero-copy loans** (`take_loan`) — sem cópia no hot path |
| **Poll loop 20ms** | **WaitSet + `take_aiter`** (async streams tokio) |
| **Caches dict+RLock global** | **`dashmap`** sharded/lock-free |
| **Métricas sem lock (bug C3)** | Atômicos — corretas por construção |
| **Gateway single-worker** | `tokio` + `Semaphore` = N workers reais |
| **Drift de tipos Py↔C++** | Tipos gerados do **IDL único** via idlc |

---

## 5. Interop & segurança da transição
- **Mesmos tópicos, mesmo IDL** → um agente Rust e três Python coexistem; ownership
  por papel (Fase 2.2, já validada no Python: cliente=10/agente=100/orq=200) arbitra.
- **Testes de contrato A/B**: a mesma bateria roda contra o nó Python e o Rust.
- **Rollback trivial**: desliga o nó Rust, os Python assumem (mesma malha DDS).

## 6. Build & verificação
```bash
cd tese/src/rust
cargo test -p qos-nfcm        # 7 testes: reproduz os números do artigo + treino paralelo
cargo check --workspace       # compila as 8 crates (sem o build C do DDS)
cargo build --release -p qos-nfcm            # binário otimizado
# quando for ligar o DDS de verdade (build C via cmake, ~minutos):
cargo build -p agent --features dds
```

## 7. Estado atual (auditoria 2026-07-17 — ver `PLANO_EXECUCAO.md`)
- ✅ **`qos-nfcm`**: implementado e testado (7 testes verdes; reproduz μ=0,923,
  w_NFIS=−0,585, Failover 0,551, margem 0,369; discrimina os 4 cenários; treino
  paralelo com rayon reduz perda e melhora acurácia; treina pertinências).
- ✅ **`orch-common`**: tipos/métricas base (mínimo: `TaskStatus` + `FuzzyMetrics`).
- ✅ **`dds-contract` (Fase 0a)**: CONCLUÍDA — 8 tipos gerados de 2 IDLs via build.rs,
  5 perfis QoS, 10+10 testes. **Lacuna:** 10 dos 17 tipos do `dds_types.py` Python
  ainda sem IDL (WF-3 do plano de execução).
- ✅ **Workspace compila** inteiro (`cargo check --workspace`).
- 🔴 **`spike-interop` (Fase 0b — GATE)**: scaffold criado (5 binários) mas **não compila
  com `--features dds` e nunca executou; nenhum número medido; gate PENDENTE**.
  ⚠️ llama-server C++ com DDS **não está buildado neste host** (build/ é artefato macOS).
- 🚧 **Scaffolds com domínio parcial** (`dds-dataspace`, `agent`, `orchestrator`,
  `llm-gateway`, `client`): lógica de domínio real, **zero integração DDS**.
- ⛔ **`llama_cpp`**: permanece C++ por decisão.

## 8. Riscos
| Risco | Nível | Mitigação |
|---|:--:|---|
| Esforço (~8,6k LOC núcleo) | médio | Incremental; cada fase entrega valor isolado |
| Bus factor da crate (autor = você) | médio | crates.io, SemVer, 256 testes; documentar na tese |
| Build C do CycloneDDS no CI | baixo | cmake presente; feature-gate mantém o check rápido |
| Curva async Rust | baixo | crate expõe streams idiomáticos; Fase 0 calibra |

## 9. Ângulo de tese
Contribuição mensurável: *“Rust vs Python para coordenação DDS em orquestração
distribuída de agentes LLM”* — deltas reais de p50/p95/p99, TTFT/ITL, throughput,
CPU e memória, no mesmo cluster e com o **mesmo NFCM** dos dois lados.
