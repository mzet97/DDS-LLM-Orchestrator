# Plano de Execução Detalhado — Migração Python → Rust

**Data:** 2026-07-17 · **Base:** auditoria estática do workspace + specs + crate `cyclonedds` + código Python.
**Escopo:** tudo que falta para migrar `src/orchestrator/` (~29k LOC Python) para `src/rust/` (~4k LOC Rust hoje), respeitando a Constituição (`specs/CONSTITUTION.md`): interop primeiro, test-first, honestidade, sem big-bang.

---

## 0. Estado de partida (confirmado na auditoria)

| Item | Estado real |
|---|---|
| `qos-nfcm` | ✅ Completo — 7 testes, NFCM/NFIS/treino rayon. **Exceto:** `src/decider.rs` órfão (não compila se incluído) |
| `orch-common` | 🟡 Mínimo — 51 LOC (`TaskStatus`, `FuzzyMetrics`); `instrumentation` vazio |
| `dds-contract` (Fase 0a) | ✅ Concluída — 8 tipos de 2 IDLs, QoS, 20 testes. Checkbox T-007 dessincronizado |
| `spike-interop` (Fase 0b) | 🔴 **Gate NÃO passou** — não compila (`DataReader<_>`), nunca rodou, zero números; checkboxes `[x]` incorretos |
| `dds-dataspace` | 🟡 Trait + mock InMemory; `DataSpace` real é casca (participante comentado) |
| `agent` | 🟡 Domínio real (Engine/claim/heartbeat), zero DDS; `main` = 1 task mock |
| `orchestrator` | 🟡 State machine + scheduler + axum funcionam, zero DDS; dep `qos-nfcm` não usada |
| `llm-gateway` | 🟡 Infra real (rate-limit/cache/Semaphore); roteamento = stub `ProviderUnavailable` |
| `client` | 🟡 HTTP real; caminho DDS = `// TODO` |
| llama-server C++ c/ DDS | ❌ **Não buildado neste host** (`src/llama_cpp/build/` é artefato macOS arm64; sem `llama-server`) |
| Contrato de tipos | ⚠️ 8 de 17 tipos — **10 tipos existem só no Python** (`dds_types.py`), sem IDL |
| Crates da dissertação | ❌ `policy-engine`, `mcp-gateway`, `context-store`, `observability`, `benchmarks` — nem criadas |
| Tasks SDD | 13 `[x]` (6 incorretos da 0b) · 34 `[ ]` |

---

## 1. Fluxo de execução (caminho crítico)

```
WF-0 saneamento ──> WF-1 infra de build ──> WF-2 Fase 0b REAL (GATE)
                                                │
                    ┌───────────────────────────┤
                    ▼                           ▼
            WF-3 contrato completo      WF-4 Fase 2 dds-dataspace
                    │                           │
                    └───────────┬───────────────┘
                                ▼
                    WF-5 Fase 1 agent (parcial ∥ WF-4)
                                ▼
                    WF-6 Fase 3 control-plane
                                ▼
                    WF-7 Fase 4 baselines + consolidação
                                ▼
                    WF-8 subsistemas da dissertação ──> WF-9 números p/ tese
```

- **WF-2 é o gate**: sem interop + benchmark medido, nenhuma fase seguinte é autorizada (ROADMAP §regra de gate).
- WF-3 e WF-4 são independentes entre si; WF-5 precisa de um dataspace mínimo (T-301/T-302/T-304).
- Orçamentos de desempenho a validar (medir, não afirmar): propagação < 5 ms p99 · ≥ 50 clientes sem deadlock · 0 gaps em ≥ 1000 chunks · ≥ 1000 tasks/s · CPU do plano de dados < Python.

---

## WF-0 — Saneamento e integridade do processo (0,5–1 dia)

Pré-condição de tudo: o SDD só funciona se o quadro de tasks disser a verdade (Art. III, VII).

| # | Ação | Arquivos | Aceite |
|---|---|---|---|
| 0.1 | Reabrir as 6 tasks da Fase 0b para `[ ]` com nota "scaffold criado; aceite não executado" | `specs/010-interop-spike/tasks.md` | Checkboxes refletem o REPORT |
| 0.2 | Marcar T-007 como `[x]` (REPORT existe) | `specs/000-dds-contract/tasks.md` | — |
| 0.3 | Atualizar tabela de fases: 000 = concluída; 010 = scaffold, gate pendente | `specs/README.md` | — |
| 0.4 | Atualizar §7 "Estado atual": `dds-contract` não é mais scaffold; spike não passou no gate | `MIGRATION_PLAN.md` | — |
| 0.5 | Resolver o órfão: mover `decider.rs` para `specs/400-baselines/` como rascunho (ou deletar) | `crates/qos-nfcm/src/decider.rs` | `cargo check -p qos-nfcm` limpo |
| 0.6 | Corrigir contagens do REPORT 0a ("10+7" → 10+10) e registrar os 2 desvios (2 IDLs, sanitização `#pragma keylist`) | `specs/000-dds-contract/REPORT.md` | — |
| 0.7 | Responder e fechar o NEEDS-CLARIFICATION da 0b: **llama-server DDS NÃO está buildado neste host** → vira WF-1.4 | `specs/010-interop-spike/spec.md` | Item resolvido na spec |
| 0.8 | Commitar ou descartar os 2 patches não commitados da crate (ver WF-1.2) | `third_party/cyclonedds-rust/` | `git status` limpo |

## WF-1 — Infraestrutura de build (1–2 dias)

| # | Ação | Arquivos | Aceite |
|---|---|---|---|
| 1.1 | Adicionar `cargo:rerun-if-env-changed=CYCLONEDDS_STATIC` ao build.rs | `third_party/cyclonedds-rust/cyclonedds-rust/cyclonedds-rust-sys/build.rs` | Alternar a var dispara rebuild sem `cargo clean` |
| 1.2 | Commitar patches locais (static + derive) e considerar release `1.8.1` no crates.io — hoje local ≠ publicado | repo da crate | Path dep e crates.io contam a mesma história |
| 1.3 | Fixar target dir fora do SMB: `CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target` (direcionar no `.cargo/config.toml` do workspace ou documentar no AGENTS.md) | `src/rust/.cargo/config.toml` | Builds sem warnings de "Permission denied"; symlinks funcionam |
| 1.4 | **Build do llama.cpp neste host (Linux)**: `cmake -B build-linux -DLLAMA_DDS=ON -DGGML_HIP=ON` (RX 7900 XTX) e `cmake --build` — gera `llama-server` com DDS | `src/llama_cpp/` | `build-linux/bin/llama-server` é ELF x86-64 e sobe com DDS |
| 1.5 | Smoke: llama-server DDS troca `LLM.InferenceRequest/Result` com o stub Python existente | `src/rust/crates/spike-interop/scripts/` | 1 request → 1 result, sem erro de matching XTypes |
| 1.6 | Validar `Dockerfile.build` como alternativa CI (build em ext4) — ou removê-lo se 1.3 resolver | `src/rust/Dockerfile.build` | Decisão registrada |

## WF-2 — Fase 0b real: interop + benchmark (GATE) — ✅ **CONCLUÍDA 2026-07-17 (PASSOU, 58×–156×)**

> **Resultado:** matriz completa (Rust↔Rust, Python↔Rust ambas direções, Rust↔C++ com
> inferência real), benchmark 10k amostras/lado (Rust p99 0,355 ms vs Python 55,46 ms),
> REPORT reescrito em `specs/010-interop-spike/REPORT.md`. Exigiu 8 correções além do
> previsto (REPORT §3): bug `DDS_OP_FLAG_KEY` no derive (crash de heap), drift IDL↔Python
> (Task/TaskOutput/SystemMetric — TypeIds agora idênticos), QoS Exclusive/strength,
> TypeInformation via blobs idlc, ktopic QoS idêntico, corrida de discovery, stubs, ambiente
> (llama-server Linux, CycloneDDS static+shared, XML do cluster sem comm local,
> `CYCLONEDDS_STATIC` com rerun-if, target fora do SMB).
> **Pendente para WF-4:** auditoria do possível double-free em `async.rs` (ver linha 118).

Reabre as tasks T-101…T-106 e executa de verdade. Só marcar `[x]` com evidência de execução.

| # | Ação | Aceite (critério objetivo) |
|---|---|---|
| 2.1 | Consertar o erro `type annotations needed for DataReader<_>` nos bins `sub-task`/`llm-client` (anotar o tipo ou reescrever contra a API atual da crate) | `CYCLONEDDS_STATIC=1 cargo build -p spike-interop --features dds` linka os 5 binários |
| 2.2 | Rust↔Rust: `pub_task` → `sub_task` no mesmo domínio (T-101) | Log de recepção com campos validados, exit 0 |
| 2.3 | Python→Rust e Rust→Python: stubs Python (`sub_task.py`/`pub_task.py` via `dds_backend`) × bins Rust (T-101/T-102) | Ambas as direções recebem `Task` com `task_id`/campos íntegros |
| 2.4 | Rust↔C++: `llm_client` ↔ `llama-server` (WF-1.4) em `LLM.InferenceRequest/Result` (T-104/REQ-103) | 1 completion real retorna; typename keyless `orchestrator::*` casa |
| 2.5 | Streaming: `pub_stream`→`sub_stream` e/ou via llama-server, ≥ 1000 chunks (T-106) | **0 gaps** de `seq_num` (exit 0 do validador) |
| 2.6 | Benchmark: criar `benches/roundtrip.rs` (criterion, estava em dev-deps sem uso) + rodar `scripts/benchmark_rtt.py`; warmup 100, ≥ 10k amostras | Tabela p50/p95/p99 + throughput **Rust e Python, mesma máquina**; `benchmark_python_results.json` + saída criterion arquivados na pasta da spec |
| 2.7 | Reescrever `specs/010-interop-spike/REPORT.md` com os números e a decisão de gate | Gate declarado: ganho ≥ 2× → Fases 1/2; < 2× → reavaliar escopo com o líder |

**Risco técnico a mitigar antes de 2.5:** o possível double-free no caminho async da crate (`async.rs`: `ptr::read` + `dds_return_loan` com tipos não-POD). O spike usa `TaskOutput` (strings). Se crashar/corromper, auditar e corrigir na crate **antes** de prosseguir — vira pré-requisito formal da WF-4/T-304.

## WF-3 — Contrato completo: os 10 tipos sem IDL — ✅ **CONCLUÍDA 2026-07-17**

> **Resultado:** `OrchestratorV4.idl` estendido de 4 → **14 tipos** (Task, AgentState,
> TaskOutput, SystemMetric + QoSRoutingProfile, ContextSnapshot, ContextUpdate,
> ToolCallRequest, ExecutionTraceEvent, SecurityPolicySnapshot, SecurityPolicyUpdate,
> QoSMetric, QoSViolation, DiscoveryEvent). `OrchestratorV4.{c,h}` regenerados via idlc.
> dds-contract: mocks dos 10 tipos, constantes de tópicos/typenames, 4 novos testes
> (`tests/contract_v4.rs` — 24 testes no total), codegen com `PartialEq`.
> **Verificação: os 14 TypeIds idlc batem byte-a-byte com os anunciados pelo Python
> em SEDP** (script de validação executado contra participante Python real).
> CONTEXT.md §3 e REPORT da Fase 0a atualizados.

Hoje só 8 dos 17 tipos do `dds_types.py` têm fonte IDL. Sem isto, Fases 3–4 e os subsistemas da dissertação não têm contrato.

| # | Ação | Aceite |
|---|---|---|
| 3.1 | Extrair campos exatos (nome, tipo, chaves) dos 10 tipos faltantes em `src/orchestrator/dds_backend/dds_types.py`: `QoSRoutingProfile`, `ContextSnapshot`, `ContextUpdate`, `ToolCallRequest`, `ExecutionTraceEvent`, `SecurityPolicySnapshot`, `SecurityPolicyUpdate`, `QoSMetric`, `QoSViolation`, `DiscoveryEvent` | Tabela campo-a-campo anexa à spec 000 |
| 3.2 | Estender `src/llama_cpp/dds/v4/idl/OrchestratorV4.idl` com os 10 structs no módulo `dds_llm_orchestrator`, `@key` idênticas ao Python | idlc C (`--check-only`) valida; parser do `cyclonedds-build` aceita (subconjunto: structs/`@key`/sequence/string — OK) |
| 3.3 | Regenerar `dds-contract`; atualizar mock types em `lib.rs` (espelho sem `dds`) | 18 tipos (8 + 10) gerados e espelhados |
| 3.4 | Teste por tipo novo: typename wire + chaves + round-trip XCDR (mesmo padrão dos 6 existentes) | +10 testes gated `dds` verdes (30 no total) |
| 3.5 | Atualizar `specs/CONTEXT.md` §3 (inventário de tópicos) e o REPORT da 0a | Documentos e código contam a mesma coisa |
| 3.6 | Regenerar o lado C (`OrchestratorV4.c/h`) e o Python (`dds_types.py` passa a ser gerado ou validado contra o IDL) — decisão: IDL vira fonte única também para o Python | Sem drift Py↔IDL (Art. I) |

## WF-4 — Fase 2: `dds-dataspace` — ✅ **CONCLUÍDA 2026-07-17**

> **Resultado:** camada DDS completa (trait `DataSpaceApi` + mock + `DataSpace` real),
> **propagação de estado p50 0,052 / p99 0,077 ms** (orçamento <5 ms, 65× abaixo; Python
> ~19 ms p50), pool de writers a **88,7k tasks/s**, caches `Arc`+DashMap sem regressão,
> streams por evento (WaitSet, sem polling), monitor de QoS com liveliness/deadline nativos,
> contract tests A/B (mock vs DDS real) verdes — 13 testes. Correções na crate no caminho:
> fix UB em `async.rs` (`ptr::read`→`clone_out` com tipos String), `Rc`→`Arc` no `Topic`
> (Send/Sync p/ tokio), caveats de `Listener` documentados. REPORT em
> `specs/200-dds-dataspace/REPORT.md`.

Onde o GIL mais dói; é pré-requisito do control plane e dá o dataspace mínimo do agente.

| Ordem | Task | Conteúdo / notas da auditoria |
|---|---|---|
| 1 | T-301 | Trait `DataSpaceApi` + `InMemoryDataSpace` — **já existe** (`api.rs`, `in_memory.rs`); falta só o teste de aceite formal → marcar e fechar |
| 2 | T-302 | Ciclo de vida do `DataSpace` real: completar a casca em `crates/dds-dataspace/src/lib.rs` (participante, 17 tópicos→18 após WF-3, shutdown ordenado) |
| 3 | T-303 | Caches: `Arc<Task>` imutável + `dashmap` — elimina as guardas anti-regressão do Python |
| 4 | **Pré-T-304** | **Auditoria de segurança do `async.rs` da crate** (double-free potencial, WF-2); corrigir e testar com tipo não-POD antes de construir streams sobre ele |
| 5 | T-304 | Streams por evento: WaitSet + `take_aiter` + loans zero-copy (mata poll loop 20 ms e churn por amostra) |
| 6 | T-305 | Pool de writers MPMC bounded (`crossbeam-channel`) + política de backpressure (mata a thread única de escrita e o `_write_queue` de 10k) |
| 7 | T-306 | Liveliness nativa (`on_liveliness_changed`) + monitor de QoS (deadline/gaps) |
| 8 | T-307 | Contract tests A/B: mesma bateria contra `InMemoryDataSpace` e `DataSpace` DDS real (lembrar `--test-threads=1` nos testes DDS) |
| 9 | T-308 | Bench de propagação + REPORT — orçamento **< 5 ms p99** (Python: 20–70 ms) |

## WF-5 — Fase 1: `agent` — ✅ **CONCLUÍDA 2026-07-18**

> **Resultado:** agente completo — claim loop sobre `stream_tasks` com confirmação de
> ownership via **estado arbitrado do RHC** (`read_task_mesh`), `DdsEngine` real contra
> o llama-server (correlação por request_id, timeout por deadline), chunks via pool de
> writers, heartbeat 5 s com uptime real, binário `--engine dds|mock`. Validação: E2E
> 10/10 tasks + 30/30 chunks + heartbeat; DdsEngine real ("Hello"); **A/B 1 Rust + 1
> Python, 100 tasks, 0 execução dupla** (arbitragem GUID-determinística). Achados
> centrais: tie-break de Exclusive Ownership por menor GUID; write-through no cache
> quebrava o readback (removido); semântica do cache == Python (anti-regressão +
> last-write-wins). Throughput claim loop: **4,02 tasks/s** (confirm sequencial 250 ms).
> REPORT em `specs/100-agent/REPORT.md`.

Substitui `src/orchestrator/agent/` (~2k LOC). Maior ROI isolado.

| Ordem | Task | Conteúdo / notas |
|---|---|---|
| 1 | T-201 | Trait `Engine` + `MockEngine` — **já existe** (`engine.rs`); falta teste de aceite → fechar |
| 2 | T-202 | Claim loop real sobre o `DataSpace`: seleção por especialização/`target_agent` (ligar `claim.rs` ao DDS) |
| 3 | T-203 | Confirmação de ownership por readback; disputa 2 agentes → exatamente 1 executa (teste cross-process) |
| 4 | T-204 | Pool MPMC de writers de `TaskOutput` (crossbeam) — hoje a publicação é `// TODO` em `agent/src/lib.rs:105` |
| 5 | T-205 | `DdsEngine`: ponte ao llama-server C++ via `LLM.Inference*` + timeout por deadline da task |
| 6 | T-206 | Heartbeat dedicado: tokio interval 5 s, Liveliness ManualByTopic; completar TODOs de `heartbeat.rs` (VRAM, uptime) |
| 7 | T-207 | Coexistência A/B: 1 agente Rust + N Python, 100 tasks, **0 execução dupla** (ownership strengths 10/100/200) |
| 8 | T-208 | Bench agente Rust vs Python (latência/throughput/CPU) + REPORT |

## WF-6 — Fase 3: control plane — ✅ **CONCLUÍDA 2026-07-18**

> **Resultado:** `orchestrator` (API axum → Tasks com strength de cliente, scheduler,
> selector, reaper por staleness de heartbeat, **loop de controle NFCM** aplicando knobs
> online com trace `qos_decision`), `client` (UM participante/N tasks — **50/50
> concorrentes sem deadlock**, 4,0 tasks/s), `llm-gateway` (pool Semaphore paralelo,
> roteamento por constraint, cache antes do rate limit, **429 retriable**), e **E2E
> Rust-only completo** (HTTP → orq → agente → llama-server C++ → "OK", latency 458 ms).
> Achados: `latency_budget` não mutável em runtime no CycloneDDS (OUT_OF_MEMORY); fix de
> deadlock em `LlmCache` (iter do DashMap durante remove). REPORT em
> `specs/300-control-plane/REPORT.md`.

| Bloco | Tasks | Notas da auditoria |
|---|---|---|
| Orchestrator | T-401–T-406 | API axum já existe (`main.rs`) mas só enfileira em memória → publicar `Task` no dataspace; ligar scheduler/registry ao DDS; **T-405 integra `qos-nfcm`** (hoje dep morta no `Cargo.toml`) com online knobs + trace `qos_decision`; state machine já tem 4 testes |
| Client | T-410–T-411 | Caminho DDS de `client/src/lib.rs:59` (1 participante para N tasks); stress **≥ 50 concorrentes** sem deadlock (Python travava em 20) |
| Gateway | T-420–T-422 | Substituir o stub `ProviderUnavailable` (`llm-gateway/src/lib.rs:194`) por roteamento local/cloud por constraint/`security_level`; cache + rate-limit (já há `RateLimiter`/`LlmCache`) + `LLMInferenceError(429, retriable)` |
| E2E | T-430 | Sistema Rust-only ponta a ponta + paridade contra cenário Python equivalente + REPORT |

## WF-7 — Fase 4: baselines + consolidação — ✅ **CONCLUÍDA 2026-07-18**

> **Resultado:** os 5 braços atrás da trait `QosDecider` (static/zadeh/fcm/fcm-dhl/nfcm);
> **zadeh e fcm reescritos como portes FIÉIS** (a versão da outra sessão era aproximação
> com erros) com **paridade verificada contra o Python** (zadeh: exato 1e-9; fcm: 1e-4,
> 7 iterações fixed_point + DHL Kosko correto); `--qos-manager` com os 5 modos no
> orchestrator; harness `five_arms` (zadeh ~190 µs × nfcm ~2 µs por decide); pacotes
> Python **arquivados** em `archive/python_qos_baselines/`; E2E Rust-only revalidado.
> **Migração do núcleo consolidada.** REPORT em `specs/400-baselines/REPORT.md`.

| # | Task | Notas |
|---|---|---|
| 1 | T-501 | Trait `QosDecider`; `Nfcm` a implementa; aproveitar o rascunho de `decider.rs` (WF-0.5) |
| 2 | T-502 | Zadeh linear — porte de `fuzzy_qos_manager/qos_selector.py` (722 LOC) **com teste de paridade** |
| 3 | T-503 | FCM + DHL — porte de `fcm_qos_manager/` (512 LOC); DHL deve divergir do linear no cenário "lote barato" |
| 4 | T-504 | `--qos-manager {static,zadeh,fcm,fcm-dhl,nfcm}` no orchestrator |
| 5 | T-505 | Harness 5 braços (`examples/five_arms.rs`) — só métricas locais medidas, sem inventar números de cluster |
| 6 | T-506 | Arquivar (não apagar) `fuzzy_qos_manager/`/`fcm_qos_manager/`/`neuro_fuzzy/` Python em `archive/` + E2E verde + REPORT final |

## WF-8 — Subsistemas da dissertação (após paridade; 3–5 semanas) — ✅ **CONCLUÍDA 2026-07-19**

> **Entregue e verde neste host** (suíte: `CYCLONEDDS_STATIC=1 cargo test -p policy-engine -p context-store -p observability -p mcp-gateway -p benchmarks --features dds -- --test-threads=1`):
> - `policy-engine` (39 testes) — rate limit funcional (bug latente do Python corrigido e documentado na NOTA DE PORTE: lá o `history.append` era inalcançável e o limite nunca negava; aqui a 1ª chamada da janela prima sem contar, as seguintes registram, `len >= max` nega, expiração faz prune+reprime).
> - `context-store` (17 testes) — `Context.Snapshot/Update` com ingestão DDS.
> - `mcp-gateway` (11 testes) — ferramentas via `ToolCall.Request` + política.
> - `observability` (15 testes) — ingestão `QoS.Metric/Violation/Discovery` + `Execution.Trace` (módulo `observability::dds`), sink JSONL, trackers com atômicos (sem o bug C3 de read-modify-write do Python).
> - `benchmarks` (18 unit + 3 loopback DDS) — gerador Poisson/bursts fiel a `real_workload_driver.py` (λ 5/15/30, burst 50 req/s × 0,5 s a cada 10 s; prompt LogNormal(ln 512, 0.5) clamp [32, 2048]), registry E1–E5/OP1–OP4 com parâmetros citados das fontes, driver que publica via `client` (strength 10) e grava JSONL no schema do `RequestRecord` (TTFT/ITL em E5; `t_*_ns` da task terminal em E1).
> - Perfis QoS do `dds-dataspace` alinhados 1:1 com o `dds_data_space.py` (Execution.Trace/Security/QoS.*/RoutingProfile — paridade de ktopic para interop).
> - Baselines (FixedRules/Mamdani/UCB1/SW-UCB) em `qos-nfcm::baselines` (WF-7, 27 testes verdes).
>
> **Fronteiras documentadas (não portadas):** análise estatística (Friedman/mixed models/Jain) permanece em `benchmarks/qualificacao/analysis/` (Python consome o JSONL); store PostgreSQL do `context-store`/`observability` virou DashMap+JSONL (follow-up); `compat-http`/`compat-grpc` permanece opcional; injeção de falha do OP3 é operacional (fora do driver).

Sem estes, a tese não fecha o desenho F23/F31/F32. Dependem de WF-3 (contrato) e WF-4 (dataspace).

| Nova crate | Substitui (Python) | LOC Py | Conteúdo |
|---|---|---:|---|
| `policy-engine` | `policy_engine/` | 255 | YAML → snapshot `SecurityPolicy` no DDS; caches locais; governança do MCP |
| `mcp-gateway` | `mcp_gateway/` | 836 | Ferramentas via `ToolCall.Request` (filesystem/github/web/db/ci-cd) + política |
| `context-store` | `context_store/` | 259 | `Context.Snapshot/Update` → PostgreSQL |
| `observability` | `observability/` + `qos_collector/` + `trace_collector/` + `metrics/` | ~870 | Ingestão `QoS.Metric/Violation/Discovery`, `Execution.Trace`, 12 condições QoS do CycloneDDS → PostgreSQL |
| `benchmarks` | `benchmarks/` | 3.098 | Geração de carga E1–E5/OP1–OP4 + análise estatística (Friedman/mixed models) |
| `compat-http`/`compat-grpc` (opcional) | baselines de transporte | — | Adapters da trait `DataSpaceApi`/`ITransport` para comparativo DDS vs HTTP vs gRPC |

Fora de escopo (Constituição Art. V): `llama_cpp` (C++), `automation/` (Ansible), `src/agent/dds_agent/` (proxy legado — bindings IDL incompatíveis; usar `proxy.py` só como referência de comportamento).

## WF-9 — Números para a tese (contínuo, consolida no fim)

1. Tabela final Rust vs Python: p50/p95/p99 de propagação, TTFT/ITL, throughput, CPU, memória — mesmo cluster, mesmo NFCM dos dois lados.
2. Reproduzir o deadlock de 20 clientes no Python e a ausência dele no Rust (≥ 50) como evidência.
3. Comparativo 5 braços de QoS (WF-7) nos cenários canônicos do artigo.
4. Alimentar `artigo_fuzzy_extension_qos/` e o capítulo da dissertação com os deltas medidos.

---

## 2. Riscos e mitigações

| Risco | Impacto | Mitigação |
|---|---|---|
| Double-free no caminho async da crate com tipos não-POD | crash/corrupção nas Fases 0b/2 | Auditoria + correção antes da T-304 (WF-2.5/WF-4.4); teste dedicado com `TaskOutput` |
| CFT da crate ≠ CFT SQL do Python | redesign se o Python usar filtro SQL | Verificar uso de `ContentFilteredTopic` no `dds_backend` ao iniciar WF-4; se houver, filtrar no consumidor |
| Build C do CycloneDDS no SMB | builds quebram/enganam | Target dir fora do SMB (WF-1.3) + `CYCLONEDDS_STATIC=1` + patch rerun-if (WF-1.1) |
| IDL parser da crate é subconjunto (sem herança/wstring/@mutable) | codegen errado silencioso | Manter IDL no subconjunto suportado; validar com idlc C `--check-only` + testes de round-trip |
| Esforço total (~8,6k LOC núcleo + ~7k subsistemas) | cronograma | Incremental por fase com gate; cada WF entrega valor isolado |
| Testes DDS precisam `--test-threads=1` | SIGSEGV esporádico em CI | Convenção documentada no AGENTS.md; wrapper `cargo test -- --test-threads=1` |

## 3. Comandos canônicos (verificação de cada WF)

```bash
cd tese/src/rust
export CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target   # WF-1.3
cargo test --workspace                                   # sem DDS (rápido)
cargo clippy --workspace -- -D warnings && cargo fmt --all --check
CYCLONEDDS_STATIC=1 cargo test -p dds-contract --features dds -- --test-threads=1
CYCLONEDDS_STATIC=1 cargo build -p spike-interop --features dds   # WF-2
cargo test -p qos-nfcm                                   # regressão do NFCM
```

## 4. Estimativa total (1 pessoa)

| Trecho | Estimativa |
|---|---|
| WF-0 + WF-1 | 2–3 dias |
| WF-2 (gate) | 3–5 dias |
| WF-3 | 2–3 dias |
| WF-4 + WF-5 | 2–4 semanas |
| WF-6 | 2–3 semanas |
| WF-7 | 1–2 semanas |
| WF-8 + WF-9 | 3–5 semanas |
| **Total** | **~3–4 meses** até consolidação completa; **paridade do núcleo (fim da WF-6) em ~6–9 semanas** |

> Regra de ouro (Art. II/III): nenhuma task vira `[x]` sem teste verde executado; nenhum número entra em relatório sem ter sido medido neste host.
