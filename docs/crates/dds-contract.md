# `dds-contract` — o contrato DDS único

> Crate: `src/rust/crates/dds-contract/` · Tipos gerados do IDL + nomes + QoS + typenames ·
> Substitui a manutenção manual de `dds_backend/dds_types.py`.

## Propósito

Centralizar o contrato DDS: os tipos vêm do **mesmo IDL** que o C++
(`OrchestratorDDS.idl`) e que o V4 (`OrchestratorV4.idl`), gerados por `build.rs` +
`cyclonedds-build` (só com feature `dds`). Sem a feature, compila **mocks manuais**
serde, com gate de paridade mock↔IDL em testes. Autoridade contratual:
`Entendimento_Tecnico_*.md` §§11–12. **19 tópicos** desde T-890
(19º `Studio.NodePresence`).

## Mapa tópico → tipo → chaves

| Tópico (`topics::`) | Tipo | Chave(s) |
|---|---|---|
| `Tasks` | `Task` | `task_id` |
| `AgentRegistry` | `AgentState` | `agent_id` |
| `TaskOutput` | `TaskOutput` | (`task_id`, `seq_num`) |
| `SystemMetrics` | `SystemMetric` | (`metric_name`, `component_id`) |
| `LLM.InferenceRequest` / `Result` / `Error`, `ServerStatus` | `LLMInference*`, `ServerStatus` | **sem chave** (correlação por `request_id`) |
| `QoS.RoutingProfile` | `QoSRoutingProfile` | `profile_id` |
| `Context.Snapshot` / `Update` | `ContextSnapshot` / `ContextUpdate` | `context_id` |
| `ToolCall.Request` | `ToolCallRequest` | `call_id` (a mesma instância evolui — sem `ToolCall.Response`) |
| `Execution.Trace` | `ExecutionTraceEvent` | (`trace_id`, `seq_num`) |
| `Security.PolicySnapshot` / `Update` | `SecurityPolicySnapshot` / `SecurityPolicyUpdate` | `policy_id` |
| `QoS.Metric` / `QoS.Violation` | `QoSMetric` / `QoSViolation` | `metric_id` / `violation_id` |
| `Studio.NodePresence` | `StudioNodePresence` | `node_id` |
| `QoS.Discovery` | `DiscoveryEvent` | `event_id` |

Typenames XTypes (`typenames::*`, 16 consts estilo `module::Struct`) fazem matching
com C++/Python; o `build.rs` injeta `#[dds_typename]` + metadados CDR extraídos do
`idlc` C, sanitiza `#pragma keylist`→`@key` e versiona a saída C
(`OrchestratorV4.c`, `OrchestratorDDS.c`) como fonte dos blobs.

## Perfis QoS (`qos::qos_profile`, espelho de `profile_mapper.py`)

| Perfil | Reliability | Durability | History | Ownership | Liveliness | Deadline | Prio |
|---|---|---|---|---|---|---|---|
| LowCost | BE | Volatile | KL1 | Exclusive | Auto 2 s | — | 0 |
| Balanced | Rel | Volatile | KL10 | Exclusive | Auto 5 s | 5 s | 1 |
| Critical | Rel | TransientLocal | KL64 | Exclusive | Auto 10 s | 2 s | 2 |
| Failover | Rel | TransientLocal | KL32 | **Shared** | Auto 1 s | 2 s | 2 |
| StreamLike | BE | Volatile | KL1 | Exclusive | **ManualByParticipant** 1 s | 1 s | 3 |

`StructuralQos` + `OnlineKnobs` (transport_priority/latency_budget_ms/
ownership_strength). `qos_profile` desconhecido → `UnknownProfile`.
`latency_budget_ms == 0` = omitido.

## API pública

- `topics::*` (19 nomes) + `topics::ALL`, `profiles::ALL` (5).
- `qos::{qos_profile, all_profiles, StructuralQos, OnlineKnobs, *Kind, UnknownProfile}`.
- `roles::{STRENGTH_CLIENT=10, STRENGTH_AGENT=100, STRENGTH_ORCHESTRATOR=200}`.
- `typenames::*`, `generated::{orchestrator::{4 tipos}, dds_llm_orchestrator::{15 tipos}}`,
  `dds::rt` (= `cyclonedds`, só com `dds`).

## Features e env

| Item | Efeito |
|---|---|
| (sem feature) | Mocks serde; gate padrão do workspace |
| `dds` | `cyclonedds` + geração IDL real no `build.rs` |
| `security` | `dds` + `cyclonedds/security` (sem uso ativo auditado aqui) |
| `CYCLONEDDS_STATIC=1` | Link estático (obrigatório em SMB/CIFS) |
| `DDS_CONTRACT_ORCHESTRATOR_IDL` | `rustc-env` com o path do IDL canônico (usado por teste-fonte) |
| `CYCLONEDDS_URI` | Não lido aqui; runtime DDS de quem consome com `dds` |

## Testes

```bash
cd src/rust
cargo test -p dds-contract                                   # mocks: unit + mock_parity (gate P1-5) + t809
CYCLONEDDS_STATIC=1 cargo test -p dds-contract --features dds -- --test-threads=1
# tipos reais: roundtrips XCDR, chaves, typenames, contract_v4, async_soundness, t808
```

## Limites

- Tipos LLM **keyless** por REQ-003 testado, mas `KeepLast(N)` global é compartilhado
  entre requests concorrentes (fragilidade documentada no próprio IDL).
- Com unificação de features (`--workspace`, via `det-responder`), o gate
  `mock_parity` é cfg'd-out; drift idlc↔IDL coberto só por compilador + testes `dds`.
- O gate anti-drift dos espelhos IDL pula silenciosamente em checkout isolado
  (só `eprintln!`) — CI standalone não trava drift `third_party/`↔`src/llama_cpp/`.
- Não há mapa único tópico→typename→QoS default no código (3 módulos separados);
  a QoS efetiva por tópico vive na dissertação (Tab.13), não neste crate.
- Feature `security` só repassa `cyclonedds/security`, sem teste/gate — não
  afirmar DDS Security ativo.
