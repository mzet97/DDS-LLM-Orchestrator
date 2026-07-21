# Contexto Completo — o que estamos migrando e por quê

> Documento de contexto para o executor. Leia **inteiro** antes de qualquer fase. Ele
> descreve o sistema atual (Python + C++), o contrato DDS, as restrições reais e o
> destino em Rust. Referências de arquivo são relativas à raiz do repo `tese/`.

## 1. O sistema em uma frase
Um **orquestrador distribuído de agentes LLM** usando **DDS** (CycloneDDS) como malha de
comunicação *data-centric*: clientes submetem tarefas, o orquestrador coordena, agentes
assumem tarefas via DDS e chamam um **`llama-server` C++** para inferência; QoS do DDS é
selecionada **adaptativamente** por um **Neuro-Fuzzy Cognitive Map (NFCM)**.

## 2. Topologia atual (origem da migração)
```
Cliente(s) ──DDS──> Orquestrador ──DDS──> Agente(s) ──DDS/FFI──> llama-server (C++)
                         │                     │
                    decisão QoS (NFCM)    heartbeat/liveliness
```
Tudo em `tese/src/`. Componentes Python (a migrar) e C++ (a manter):

| Caminho Python | Papel | LOC~ | Destino Rust |
|---|---|---:|---|
| `src/orchestrator/agent/` | assume tasks PENDING, ponte com llama-server, streaming | 2,0k | crate `agent` |
| `src/orchestrator/dds_backend/` | camada DDS: readers/writers, poll loop, caches, QoS monitor | 3,4k | crate `dds-dataspace` |
| `src/orchestrator/orchestrator/` | scheduler, registry, selector, dispatcher, state machine, control loop | 2,0k | crate `orchestrator` |
| `src/orchestrator/client/` | submete tasks, recebe resultados/streaming | 0,2k | crate `client` |
| `src/orchestrator/llm_gateway/` | roteia a provedores (local/cloud), cache, rate-limit | 1,0k | crate `llm-gateway` |
| `src/orchestrator/neuro_fuzzy/` | **NFCM** (decisão de QoS) — já portado | 1,0k | crate `qos-nfcm` ✅ |
| `src/orchestrator/fuzzy_qos_manager/`, `fcm_qos_manager/` | baselines Zadeh/FCM/DHL | 1,2k | dentro de `qos-nfcm` (Fase 4) |
| `src/orchestrator/common/` | instrumentação, logging | 0,3k | crate `orch-common` (base ✅) |
| `src/llama_cpp/` | **motor de inferência** (+ bridge DDS) | — | **MANTÉM C++** |
| `src/automation/` | Ansible | — | **fora de escopo** |

## 3. O contrato DDS (a fonte da verdade)
- IDLs canônicos: **`src/llama_cpp/dds/idl/OrchestratorDDS.idl`** (tipos LLM, interop C++)
  e **`src/llama_cpp/dds/v4/idl/OrchestratorV4.idl`** (tipos v4/plataforma). O C++ gera
  deles; o Rust gera deles via `cyclonedds-build` (build.rs da `dds-contract`); o Python
  tem `dds_types.py` **manual** (fonte de *drift* histórico — ver nota abaixo).
- **Atualizado em 2026-07-17 (WF-3):** o V4 agora cobre os **14 tipos** que o Python
  define (antes só 4). Foi preciso corrigir **drift real**: `Task` +7 campos
  (`target_agent`, 6× `t_*_ns`), `TaskOutput` +2 (`agent_id`, `token_count`),
  `SystemMetric.value` `double`→`float`. **Os TypeIds idlc batem byte-a-byte com os
  anunciados pelo Python em SEDP — verificado nos 14 tipos.**
- Tópicos canônicos e tipos-chave:
  | Tópico | Tipo | Chave (@key) | Observação |
  |---|---|---|---|
  | `Tasks` | `Task` | `task_id` | Ownership.Exclusive (strength por papel); ciclo PENDING→ASSIGNED→RUNNING→DONE/FAILED |
  | `AgentRegistry` | `AgentState` | `agent_id` | Shared; Liveliness (heartbeat) |
  | `TaskOutput` | `TaskOutput` | `task_id, seq_num` | Exclusive; streaming; deadline 10 s |
  | `SystemMetrics` | `SystemMetric` | `metric_name, component_id` | métricas de sistema |
  | `QoS.RoutingProfile` | `QoSRoutingProfile` | `profile_id` | perfil de roteamento QoS |
  | `Context.Snapshot` | `ContextSnapshot` | `context_id` | snapshot de contexto |
  | `Context.Update` | `ContextUpdate` | `context_id` | delta de contexto |
  | `ToolCall.Request` | `ToolCallRequest` | `call_id` | ferramentas MCP |
  | `Execution.Trace` | `ExecutionTraceEvent` | `trace_id, seq_num` | tracing de execução |
  | `Security.PolicySnapshot` | `SecurityPolicySnapshot` | `policy_id` | política vigente |
  | `Security.PolicyUpdate` | `SecurityPolicyUpdate` | `policy_id` | delta de política |
  | `QoS.Metric` | `QoSMetric` | `metric_id` | métricas de experimentos |
  | `QoS.Violation` | `QoSViolation` | `violation_id` | violações de QoS |
  | `QoS.Discovery` | `DiscoveryEvent` | `event_id` | eventos de discovery |
  | `LLM.InferenceRequest/Result/Error` | `LLM*` | **keyless** | interop com o C++ (typenames `orchestrator::…`) |
  | `ServerStatus` | `ServerStatus` | **keyless** | heartbeat do llama-server |
- **Reconciliação já feita (Python):** os 3 tipos LLM são **keyless** e com typename
  `orchestrator::…` para casar o XTypes com o C++. Um teste
  (`tests/test_idl_python_consistency.py`) valida Python↔IDL. **Em Rust, o idlc elimina
  esse risco por construção** — não repita a manutenção manual.
- **Regra de ouro:** todo tipo novo entra primeiro no IDL (com teste de round-trip e
  verificação de TypeId contra o Python); nunca nasce só no Python ou só no Rust.

## 4. Os gargalos que motivam a migração (confirmados no código)
| # | Gargalo | Evidência | Alvo Rust |
|---|---|---|---|
| G1 | **GIL** serializa tudo (server async + poll + write + conversões) | processo único Python | sem GIL |
| G2 | **Deadlock ≥20 clientes** | cada `DDSClient` cria um `DDSDataSpace` (17 tópicos+threads); 20 = 20 participantes × GIL; OP1 travado em 20 | 1 participante/N tasks async |
| G3 | **Thread única de escrita** | `_write_queue` (maxsize 10k) drenada por 1 `dds-write-loop` | pool de writers MPMC |
| G4 | **Churn por amostra** | `take(N=64)` + `dds_to_task` constrói objetos por amostra | zero-copy loans |
| G5 | **Poll loop 20ms** | `_poll_loop` itera 16 readers (mitigado por WaitSet híbrido no Py) | WaitSet + async streams |
| G6 | **Caches dict + RLock global** | leitura de agente serializa com escrita de task | `dashmap` sharded |
| G7 | **Métricas sem lock (bug C3)** | RTTTracker/ErrorTracker read-modify-write | atômicos |
| G8 | **Gateway single-worker** | `for i in range(1)`; paralelismo bloqueado pelo GIL | `tokio`+`Semaphore` |

## 5. Restrições REAIS do DDS (não violar — herdado do plano Python)
- **Mutáveis em runtime** no CycloneDDS avaliado: **TransportPriority, LatencyBudget,
  OwnershipStrength**. Só essas o decisor de QoS altera "quente".
- **Deadline** retorna *unsupported* em runtime; **Reliability/Durability/History** são
  **imutáveis** após criar a entidade → decisões **estruturais** (por tópico, na criação).
- A saída do decisor de QoS separa **online** (mutáveis) de **estrutural** (recriação).
- **Ownership por papel (Fase 2.2, já validada no Python):** cliente=10, agente=100,
  orquestrador=200. O middleware arbitra o claim; readback confirma (`confirm_task_claim`).
- **Liveliness (Fase 2.4):** AgentRegistry writer usa `ManualByTopic(lease 10s)`;
  heartbeat dedicado escreve a cada 5s (<lease). **Atenção:** o listener nativo
  `on_liveliness_changed` foi EVITADO no Python por deadlock de GIL — **em Rust o listener
  nativo é seguro** e deve ser usado.

## 6. O NFCM (já portado — referência de paridade)
- Pipeline: métricas(8) → fuzificação gaussiana treinável → NFIS ajusta pesos causais →
  nós internos (h_pressure/h_health/h_stream) iteram com realimentação real → softmax(5 perfis).
- Perfis: `QoS_Critical/Failover/StreamLike/LowCost/Balanced`.
- 8 métricas (ordem canônica): urgency, deadline_pressure, recent_latency, agent_load,
  error_rate, historical_confidence, estimated_complexity, streaming_need.
- **Números de paridade** (cenário degradado): μ_alto(error_rate=0.90)=0.923; peso NFIS
  ajustado=−0.585; h_health≈0.002; h_pressure≈0.712; Failover softmax=0.551; margem=0.369;
  converge em ~4 iterações. Discrimina os 4 cenários canônicos.
- Já em Rust: crate `qos-nfcm` (7 testes verdes). **Referência de como portar com paridade.**

## 7. A crate DDS em Rust (a ferramenta)
`third_party/cyclonedds-rust/cyclonedds-rust/cyclonedds` — v1.8.0, crates.io, 256 testes,
autoria do próprio autor da tese. Capacidades relevantes:
- Modelo DDS completo; **26+ QoS via `QosBuilder`**; **13 listeners** (`ListenerBuilder`).
- **`cyclonedds-idlc`**: compila `.idl` → Rust (`--input <idl> --output-dir <dir>`).
- Derive macros `DdsType/DdsEnum/DdsUnion/DdsBitmask`; **XCDR1/XCDR2**, XTypes.
- **Async streams** (`read_aiter`/`take_aiter`) com tokio; timeouts/cancelamento.
- **Zero-copy loans** (`write_loan`/`read_loan`/`take_loan`).
- WaitSet/ReadCondition/QueryCondition/GuardCondition; Security X.509; tracing; Prometheus.
- Referenciar via path no workspace; feature `dds` liga o build (CycloneDDS via cmake).

## 8. Hardware alvo
- **Ryzen 9 5900X (12c/24t)** → `tokio` multi-thread runtime + `rayon` para data-parallel.
- **64 GB RAM** → caches e loans sem pressão.
- **RX 7900 XTX 24GB (ROCm)** → do `llama_cpp` (C++). Futuro opcional: treino NFCM em GPU
  (`candle`/`burn` ROCm). **Não** é requisito da migração.

## 9. Estado do workspace Rust (ponto de partida do executor)
- `cargo check --workspace` ✓ (8 crates). `cargo test -p qos-nfcm` ✓ (7 testes).
- Implementado: `qos-nfcm`, `orch-common` (base). Scaffolds: as demais (com doc comments
  de mapeamento e dep `cyclonedds` opcional).
- Convenções: edição 2021, rust >=1.85 (ambiente tem 1.95), perfil release com LTO.

## 10. Onde encontrar mais
- **Arquitetura autoritativa do autor (LEIA):** `specs/DISSERTACAO.md` — 4 planos, 11 tópicos,
  subsistemas (inclui policy-engine/mcp-gateway/context-store/observability que faltavam aqui),
  abstração de transporte, implantação (a RX 7900 XTX roda agente+gateways), estado de implementação.
- Catálogo de figuras: `specs/FIGURES.md` (com alerta de descasamento arquivo↔legenda).
- Plano macro: `src/rust/MIGRATION_PLAN.md`.
- Handoff de cluster (execução remota): `opencode_deve_fazer.md`.
- Plano de correção Python (histórico das fases 1–4): `docs/planning/PLANO_ACAO_CORRECAO_2026-07.md`.
- Artigo NFCM: `artigo_fuzzy_extension_qos/paper/`.
