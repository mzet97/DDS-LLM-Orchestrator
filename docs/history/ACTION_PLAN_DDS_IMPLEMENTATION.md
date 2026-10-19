# Plano de Ação — Implementação Real com DDS

**Versão:** 1.0
**Data:** 2026-07-15
**Autor:** Principal SWE
**Objetivo:** Implementar todas as funcionalidades da migração Python → Rust usando DDS real (cyclonedds-rust)

---

## Princípios

1. **DDS real, não mocks** — Todo código deve funcionar com `--features dds`
2. **Test-first** — Cada task tem teste unitário em `tests/` antes da implementação
3. **Zero-copy no hot path** — Usar `write_loan`/`take_loan` para streaming
4. **Arc<Task>** — Compartilhar tasks via Arc, não clone
5. **Performance medida** — Cada componente tem benchmark

---

## Fase 1: Tipos DDS Completos (dds-contract)

**Objetivo:** Gerar todos os 17 tipos DDS do IDL canônico.

### Tasks

---

## Fase 1: Verificação dos Tipos DDS ✅ Já Completo

**Status:** Os 18 tipos (14 V4 + 4 LLM) já estão gerados do IDL e testados.

O `contract_v4.rs` já testa:
- Typenames corretos para todos os 18 tipos
- Keys corretas para todos os tipos
- XCDR1 round-trip para todos os tipos
- Type metadata blobs para todos os tipos

**Ação:** Nenhuma — pular para Fase 2.

---

## Fase 2: DDS Dataspace Completo (dds-dataspace)

**Objetivo:** Implementar todos os 17 tópicos com streams, caches e writers.

### Tasks

#### T-610: Tópicos LLM (InferenceRequest/Result/Error)
- **Req:** REQ-301, REQ-302
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:**
  - Criar topics, writers, readers para `LLM.InferenceRequest`, `LLM.InferenceResult`, `LLM.InferenceError`
  - Adicionar streams `stream_llm_requests()`, `stream_llm_results()`
  - Adicionar caches `llm_requests_cache`, `llm_results_cache`
- **Teste:** `tests/llm_topics.rs` — write request → stream receives → cache updated
- **Aceite:** LLM request/response flow funciona via DDS

#### T-611: Tópicos Context (Snapshot/Update)
- **Req:** REQ-301
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:**
  - Criar topics, writers, readers para `Context.Snapshot`, `Context.Update`
  - Adicionar streams `stream_context_snapshots()`, `stream_context_updates()`
- **Teste:** `tests/context_topics.rs` — write snapshot → update → stream receives both
- **Aceite:** Context flow funciona via DDS

#### T-612: Tópicos ToolCall
- **Req:** REQ-301
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:** Criar topic, writer, reader para `ToolCall.Request`
- **Teste:** `tests/toolcall_topics.rs`
- **Aceite:** ToolCall request/response funciona

#### T-613: Tópicos ExecutionTrace
- **Req:** REQ-301
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:** Criar topic, writer para `Execution.Trace`
- **Teste:** `tests/trace_topics.rs`
- **Aceite:** Trace events são publicados

#### T-614: Tópicos SecurityPolicy
- **Req:** REQ-301
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:** Criar topics, writers, readers para `Security.PolicySnapshot`, `Security.PolicyUpdate`
- **Teste:** `tests/security_topics.rs`
- **Aceite:** Policy snapshot/update funciona

#### T-615: Tópicos QoS Monitoring
- **Req:** REQ-308
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:** Criar topics, writers para `QoS.Metric`, `QoS.Violation`, `QoS.Discovery`
- **Teste:** `tests/qos_monitoring_topics.rs`
- **Aceite:** QoS metrics são publicados

#### T-616: Zero-copy writes para streaming
- **Req:** REQ-303
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:**
  - Substituir `writer.write(&task)` por `writer.request_loan()` + `WriteLoan::write()` no hot path
  - Implementar `write_task_loan()`, `write_output_loan()`
- **Teste:** `tests/zero_copy.rs` — comparar latência write vs write_loan
- **Aceite:** Zero-copy funciona, latência medida

#### T-617: Shared WaitSet com ReadConditions
- **Req:** REQ-302
- **Arquivo:** `crates/dds-dataspace/src/lib.rs`
- **Ação:**
  - Criar um WaitSet compartilhado com ReadCondition por tópico
  - Substituir um reader por stream com `take_aiter_batch`
- **Teste:** `tests/shared_waitset.rs` — múltiplos tópicos acordam o mesmo WaitSet
- **Aceite:** 17 tópicos compartilham 1 WaitSet

#### T-618: Cache com ahash
- **Req:** REQ-304
- **Arquivo:** `crates/dds-dataspace/src/cache.rs`
- **Ação:** Substituir hasher padrão por `ahash::AHasher` nos DashMaps
- **Teste:** `tests/cache_hasher.rs` — benchmark de throughput
- **Aceite:** Throughput medido, melhoria documentada

#### T-619: Liveliness listener nativo
- **Req:** REQ-307
- **Arquivo:** `crates/dds-dataspace/src/monitor.rs`
- **Ação:**
  - Implementar `on_liveliness_changed` listener no reader de AgentRegistry
  - Detectar agentes mortos por listener (não polling)
- **Teste:** `tests/liveliness.rs` — matar agente, verificar detecção
- **Aceite:** Liveliness lost detectado em < 2s

#### T-620: Deadline missed listener
- **Req:** REQ-308
- **Arquivo:** `crates/dds-dataspace/src/monitor.rs`
- **Ação:**
  - Implementar `on_requested_deadline_missed` listener
  - Publicar `QoS.Violation` quando deadline perdido
- **Teste:** `tests/deadline.rs` — forçar deadline miss, verificar violação publicada
- **Aceite:** Deadline miss detectado e publicado

#### T-621: Contract tests A/B (mock vs DDS)
- **Req:** REQ-309
- **Arquivo:** `crates/dds-dataspace/tests/contract.rs`
- **Ação:** Parametrizar testes para rodar com `InMemoryDataSpace` e `DataSpace` DDS real
- **Teste:** `tests/contract.rs` — mesma bateria nos dois backends
- **Aceite:** Todos os testes passam nos dois backends

---

## Fase 3: Agent Completo (agent)

**Objetivo:** Implementar agente que funciona com DDS real.

### Tasks

#### T-630: DdsEngine completo
- **Req:** REQ-203
- **Arquivo:** `crates/agent/src/engine_dds.rs`
- **Ação:**
  - Implementar `DdsEngine` que publica `LLMInferenceRequest` e recebe `LLMInferenceResult`
  - Usar `take_aiter` para receber resultados (não polling)
  - Timeout derivado do deadline da task
- **Teste:** `tests/engine_dds.rs` — mock llama-server, verificar request→result
- **Aceite:** DdsEngine funciona com DDS real

#### T-631: Claim loop com readback confirmation
- **Req:** REQ-201, REQ-202
- **Arquivo:** `crates/agent/src/dds.rs`
- **Ação:**
  - Implementar claim loop que assina Tasks via stream
  - Confirmar ownership via `read_task_mesh()` (RHC do reader)
  - Usar `write_task()` com ownership strength de agente (100)
- **Teste:** `tests/claim.rs` — 2 agentes disputam 1 task, só 1 executa
- **Aceite:** Zero execução dupla

#### T-632: TaskOutput streaming com writer pool
- **Req:** REQ-204
- **Arquivo:** `crates/agent/src/dds.rs`
- **Ação:**
  - Usar `WriterPool` para publicar chunks de TaskOutput
  - Usar `write_loan()` para zero-copy no hot path
  - seq_num crescente, sem gaps
- **Teste:** `tests/streaming.rs` — 1000 chunks, 0 gaps
- **Aceite:** Streaming funciona com zero-copy

#### T-633: Heartbeat com assert_liveliness
- **Req:** REQ-205
- **Arquivo:** `crates/agent/src/heartbeat.rs`
- **Ação:**
  - Publicar `AgentState` a cada 5s
  - Chamar `writer.assert_liveliness()` para ManualByTopic
  - VRAM detection (se possível)
- **Teste:** `tests/heartbeat.rs` — verificar publicação periódica
- **Aceite:** Heartbeat publica durante inferência longa

#### T-634: E2E agent test com DDS real
- **Req:** REQ-208
- **Arquivo:** `crates/agent/tests/agent_e2e.rs`
- **Ação:**
  - Teste E2E: 10 tasks claim→DONE, 30 chunks, heartbeat
  - Usar `DataSpace` DDS real (não mock)
- **Teste:** `tests/agent_e2e.rs`
- **Aceite:** E2E passa com DDS real

---

## Fase 4: Orchestrator Completo (orchestrator)

**Objetivo:** Implementar orchestrator que funciona com DDS real.

### Tasks

#### T-640: OrchestratorDds com todos os tópicos
- **Req:** REQ-401, REQ-402, REQ-403
- **Arquivo:** `crates/orchestrator/src/dds.rs`
- **Ação:**
  - Implementar `OrchestratorDds` que usa `DataSpace` DDS real
  - Publicar tasks com ownership strength de orchestrator (200)
  - Assinar AgentRegistry para registry
  - Assinar TaskOutput para consolidação
- **Teste:** `tests/dds_orchestrator.rs` — publicar task, verificar no DDS
- **Aceite:** Orchestrator publica e lê via DDS

#### T-641: Reaper de tasks expiradas
- **Req:** REQ-403
- **Arquivo:** `crates/orchestrator/src/dds.rs`
- **Ação:
  - Implementar reaper que detecta tasks PENDING com deadline expirado
  - Reatribuir ou falhar tasks
- **Teste:** `tests/reaper.rs` — criar task com deadline curto, verificar reatribuição
- **Aceite:** Tasks expiradas são reatribuídas

#### T-642: Control loop com NFCM
- **Req:** REQ-405
- **Arquivo:** `crates/orchestrator/src/dds.rs`
- **Ação:**
  - Implementar control loop que coleta métricas do DDS
  - Chamar `Nfcm::infer()` para decisão de QoS
  - Aplicar online knobs via `apply_tasks_knobs()`
  - Publicar trace `qos_decision`
- **Teste:** `tests/control_loop.rs` — métricas degradadas → Failover
- **Aceite:** NFCM integra com DDS real

#### T-643: Fuzzy routing publication
- **Req:** REQ-405
- **Arquivo:** `crates/orchestrator/src/dds.rs`
- **Ação:**
  - Publicar `QoS.RoutingProfile` quando perfil muda
  - Incluir weighted agent prefixes
- **Teste:** `tests/fuzzy_routing.rs` — perfil muda → routing publicado
- **Aceite:** Routing profile publicado via DDS

#### T-644: axum API com DDS backend
- **Req:** REQ-401
- **Arquivo:** `crates/orchestrator/src/main.rs`
- **Ação:**
  - Conectar axum API ao `OrchestratorDds`
  - POST /api/v1/chat/completions → criar task → publicar via DDS
  - GET /api/v1/agents → ler do cache de AgentRegistry
- **Teste:** `tests/api.rs` — POST task, verificar no DDS
- **Aceite:** API funciona com DDS real

---

## Fase 5: Client Completo (client)

**Objetivo:** Implementar cliente que submete via DDS.

### Tasks

#### T-650: DdsClientDds com submit e stream
- **Req:** REQ-410
- **Arquivo:** `crates/client/src/lib.rs`
- **Ação:**
  - Implementar `DdsClientDds` que publica task via DDS
  - Assinar TaskOutput para receber resultados
  - Implementar `submit_stream()` que retorna Stream de chunks
- **Teste:** `tests/client_dds.rs` — submit → receber resultado
- **Aceite:** Client submete e recebe via DDS

#### T-651: Stress test 50+ concorrentes
- **Req:** REQ-411
- **Arquivo:** `crates/client/tests/client.rs`
- **Ação:**
  - Teste de 50+ submits concorrentes
  - Verificar zero deadlock
- **Teste:** `tests/client.rs` — 50 submits paralelos
- **Aceite:** 50+ concorrentes sem deadlock

---

## Fase 6: LLM Gateway Completo (llm-gateway)

**Objetivo:** Implementar gateway com providers reais.

### Tasks

#### T-660: LlmProvider trait com providers reais
- **Req:** REQ-421
- **Arquivo:** `crates/llm-gateway/src/lib.rs`
- **Ação:**
  - Implementar `LocalProvider` que publica `LLMInferenceRequest` via DDS
  - Implementar `OpenRouterProvider` que faz HTTP para OpenRouter
  - Roteamento por constraint (LOCAL_ONLY/ANY)
- **Teste:** `tests/providers.rs` — local provider → DDS → resultado
- **Aceite:** Providers funcionam com DDS

#### T-661: Cache com Redis
- **Req:** REQ-422
- **Arquivo:** `crates/llm-gateway/src/lib.rs`
- **Ação:**
  - Implementar `RedisCache` para resultados LLM
  - Cache key = hash(prompt + model)
- **Teste:** `tests/cache.rs` — hit devolve resultado
- **Aceite:** Cache funciona com Redis

#### T-662: Rate limiter com 429
- **Req:** REQ-422
- **Arquivo:** `crates/llm-gateway/src/lib.rs`
- **Ação:**
  - Rate limiter já implementado, verificar com DDS
  - Drop por rate-limit publica `LLMInferenceError(429, retriable)`
- **Teste:** `tests/rate_limit.rs` — flood → 429 errors
- **Aceite:** Rate limiting funciona

#### T-663: Worker pool com Semaphore
- **Req:** REQ-420
- **Arquivo:** `crates/llm-gateway/src/lib.rs`
- **Ação:**
  - Worker pool já implementado, verificar com DDS
  - N workers processam em paralelo
- **Teste:** `tests/worker_pool.rs` — N workers processam em paralelo
- **Aceite:** Worker pool funciona com DDS

---

## Fase 7: Subsistemas Adicionais

**Objetivo:** Implementar subsistemas Python sem counterpart Rust.

### Tasks

#### T-670: Policy Engine crate
- **Req:** Novo
- **Arquivo:** `crates/policy-engine/`
- **Ação:**
  - Criar crate `policy-engine`
  - Implementar `PolicyEngine` que carrega policies.json
  - Publicar `SecurityPolicySnapshot` via DDS
  - Avaliar políticas localmente
- **Teste:** `tests/policy.rs` — carregar policy, publicar, avaliar
- **Aceite:** Policy engine funciona com DDS

#### T-671: MCP Gateway crate
- **Req:** Novo
- **Arquivo:** `crates/mcp-gateway/`
- **Ação:**
  - Criar crate `mcp-gateway`
  - Implementar `McpGateway` que assina `ToolCall.Request`
  - Roteamento para filesystem, GitHub, web clients
  - Aplicar policy engine
- **Teste:** `tests/mcp.rs` — tool call → resultado
- **Aceite:** MCP gateway funciona com DDS

#### T-672: Context Store crate
- **Req:** Novo
- **Arquivo:** `crates/context-store/`
- **Ação:**
  - Criar crate `context-store`
  - Implementar `ContextStore` que assina `Context.Update`
  - Persistir em PostgreSQL
- **Teste:** `tests/context_store.rs` — update → persistir → ler
- **Aceite:** Context store funciona com DDS

#### T-673: Observability crate
- **Req:** Novo
- **Arquivo:** `crates/observability/`
- **Ação:**
  - Criar crate `observability`
  - Implementar coletores para QoS/Trace/Metrics
  - Persistir em PostgreSQL/JSONL
- **Teste:** `tests/observability.rs` — evento → persistir
- **Aceite:** Observability funciona com DDS

#### T-674: Metrics crate
- **Req:** Novo
- **Arquivo:** `crates/orch-common/src/metrics.rs`
- **Ação:**
  - Implementar `TokenCounter`, `CostTracker`, `RttTracker`
  - Contadores atômicos (thread-safe)
- **Teste:** `tests/metrics.rs` — contar tokens, calcular custo
- **Aceite:** Metrics funcionam

---

## Fase 8: Benchmarks e Integração

**Objetivo:** Medir performance e validar integração E2E.

### Tasks

#### T-680: Benchmark de propagação de estado
- **Req:** REQ-310
- **Arquivo:** `crates/dds-dataspace/benches/propagation.rs`
- **Ação:**
  - Medir latência de propagação Task (write → stream receives)
  - Comparar com Python (p99 < 5ms target)
- **Teste:** Criterion benchmark
- **Aceite:** p99 medido e documentado

#### T-681: Benchmark de streaming
- **Req:** REQ-209
- **Arquivo:** `crates/agent/benches/streaming.rs`
- **Ação:**
  - Medir latência de streaming (write_loan vs write)
  - Medir throughput de chunks
- **Teste:** Criterion benchmark
- **Aceite:** Latência e throughput medidos

#### T-682: Benchmark de writer pool
- **Req:** REQ-305
- **Arquivo:** `crates/dds-dataspace/benches/writer_pool.rs`
- **Ação:**
  - Medir throughput do writer pool
  - Comparar com thread única
- **Teste:** Criterion benchmark
- **Aceite:** Throughput medido

#### T-683: E2E Rust-only test
- **Req:** REQ-430
- **Arquivo:** `crates/orchestrator/tests/e2e.rs`
- **Ação:**
  - Teste E2E: cliente → orchestrator → agente → resultado
  - Tudo Rust, sem Python
- **Teste:** `tests/e2e.rs`
- **Aceite:** E2E passa

#### T-684: A/B test Rust vs Python
- **Req:** Art. I
- **Arquivo:** `tests/ab_comparison.rs`
- **Ação:**
  - Mesma carga contra Rust e Python
  - Comparar latência, throughput, CPU
- **Teste:** Benchmark comparativo
- **Aceite:** Resultados documentados

---

## Fase 9: Consolidação e Documentação

**Objetivo:** Documentar estado final e arquivar Python.

### Tasks

#### T-690: REPORT final da migração
- **Req:** Art. VII
- **Arquivo:** `specs/REPORT_FINAL.md`
- **Ação:**
  - Documentar todas as crates implementadas
  - Listar todos os testes passando
  - Incluir benchmarks
  - Comparar com Python
- **Aceite:** REPORT existe e é honesto

#### T-691: Arquivar Python equivalente
- **Req:** REQ-506
- **Arquivo:** `archive/python-migration/`
- **Ação:**
  - Mover módulos Python migrados para archive
  - Adicionar nota de arquivamento
  - Manter apenas módulos não migrados ativos
- **Aceite:** Python arquivado, Rust é o default

#### T-692: Atualizar MIGRATION_PLAN.md
- **Req:** Art. VII
- **Arquivo:** `MIGRATION_PLAN.md`
- **Ação:**
  - Atualizar status de todas as crates
  - Documentar decisões tomadas
  - Listar work restante (se houver)
- **Aceite:** Plano reflete realidade

---

## Resumo de Fases

| Fase | Foco | Tasks | Dependências | Status |
|------|------|-------|-------------|--------|
| 1 | Tipos DDS completos | 0 | Nenhuma | ✅ Já completo |
| 2 | DDS Dataspace completo | 12 | Fase 1 | Pendente |
| 3 | Agent completo | 5 | Fase 2 | Pendente |
| 4 | Orchestrator completo | 5 | Fase 2 | Pendente |
| 5 | Client completo | 2 | Fase 2 | Pendente |
| 6 | LLM Gateway completo | 4 | Fase 2 | Pendente |
| 7 | Subsistemas adicionais | 5 | Fase 2 | Pendente |
| 8 | Benchmarks e integração | 5 | Fases 3-6 | Pendente |
| 9 | Consolidação | 3 | Todas | Pendente |
| **Total** | | **41** | | |

---

## Orçamento de Tempo

| Fase | Estimativa |
|------|-----------|
| 1 | ✅ Completo |
| 2 | 8-12 horas |
| 3 | 4-6 horas |
| 4 | 4-6 horas |
| 5 | 2-3 horas |
| 6 | 3-4 horas |
| 7 | 6-8 horas |
| 8 | 4-6 horas |
| 9 | 2-3 horas |
| **Total** | **33-48 horas** |

---

## Critérios de Saída

Cada fase tem gate:
- Todos os testes passam (`cargo test --features dds`)
- `cargo clippy -- -D warnings` limpo
- `cargo fmt --check` limpo
- Benchmarks rodam e resultados documentados
- REPORT.md da fase escrito

O gate final:
- E2E Rust-only funciona
- A/B test Rust vs Python documentado
- Python equivalente arquivado
- REPORT final honesto
