# Optimization Audit — DDS-LLM Orchestrator (Rust workspace)

**Data:** 2026-07-20 · **Autor:** Principal SWE / Performance Engineer (auditoria, sem alterações de código)
**Escopo:** `tese/src/rust/` (14 crates) — análise estática de código-fonte + execução dos
comandos de validação do workspace. Nenhum arquivo de crate foi modificado nesta sessão.

**Baseline conhecido (medido, ver `PLANO_EXECUCAO.md`):**

| Métrica | Valor |
|---|---:|
| Propagação de estado DDS (p50/p99) | 0,052 ms / 0,077 ms |
| Throughput do writer pool | 88.752 tasks/s |
| Clientes concorrentes (submit) | 50/50, 0 deadlock |
| E2E (HTTP→orq→agente→llama-server→resultado) | ~458 ms |

---

## 0. Achado pós-auditoria (2026-07-20): fonte do IDL C++ apontava para árvore obsoleta

**Contexto:** o usuário informou que `tese/third_party/llama.cpp_dds/` — e não
`tese/src/llama_cpp/` — é a árvore atual da integração C++/DDS. `dds-contract/build.rs`
resolvia os caminhos do IDL relativos a `src/llama_cpp/`.

**Investigação (confirmada por leitura direta, não por suposição):**
- `dds/idl/OrchestratorDDS.idl` (módulo `orchestrator`: `LLMInferenceRequest/Result/Error`,
  `ServerStatus`) — **idêntico** nas duas árvores (`diff` sem saída). O `.c` gerado também é
  idêntico byte-a-byte (`cmp` limpo). **Sem risco** — é o único IDL que o bridge C++ do
  `llama-server` de fato usa (confirmado por grep: `dds_bridge.cpp`/`.h` e
  `dds_transport.cpp`/`.h` de `third_party/llama.cpp_dds/dds/` não referenciam nenhum tipo
  de `OrchestratorV4`).
- `dds/v4/idl/OrchestratorV4.idl` (módulo `dds_llm_orchestrator`: Task/AgentState/
  TaskOutput/SystemMetric + os 10 tipos da WF-3) — **divergente**: `third_party/llama.cpp_dds`
  tinha a versão **pré-WF-3** (4 tipos, `#pragma keylist`, sem os 6 campos de instrumentação
  de latência em `Task`, sem `target_agent`, sem `agent_id`/`token_count` em `TaskOutput`,
  `SystemMetric.value` como `double` em vez de `float`, e faltando os 10 structs novos
  inteiros). O `.c` compilado confirmava: 0 ocorrências de `QoSRoutingProfile`. mtime do
  arquivo em `third_party`: 2026-07-20 13:42 (hoje); mtime em `src/llama_cpp`: 2026-07-17
  21:44 (data da WF-3) — consistente com `third_party/llama.cpp_dds` sendo uma cópia/checkout
  mais recente que **predata** o trabalho da WF-3, e não uma evolução independente dela. O
  comentário do arquivo gerado também mostrava `Source: /Users/zeitune/Documents/tese/...`
  (caminho macOS) e `Cyclone DDS: V0.11.0`, contra `V11.0.1` na árvore `src/llama_cpp` —
  reforça que são checkouts de origens/momentos diferentes.
- `third_party/llama.cpp_dds/dds/v4/dds_v4_bridge.cpp` (o bridge C++ que de fato usa os
  tipos V4) **não referencia nenhum dos campos/tipos que só existiam na versão nova**
  (confirmado por grep) — logo os campos adicionais não quebram esse arquivo (adição de
  campo é compatível). A única mudança não-aditiva é `SystemMetric.value`: `double`→`float`;
  o bridge tem `void publish_metric(..., double value, ...)` e faz `m.value = value` — vira
  uma conversão estreitando `double`→`float` implícita, compila (no máximo warning de
  `-Wfloat-conversion` se o build usar essa flag; não é erro). Não alterado nesta sessão
  (fora do escopo pedido — só o IDL foi portado).
- Também notado: `third_party/llama.cpp_dds/dds/idl/LlamaDDS.idl` é um módulo novo (schema
  de chat completion estilo OpenAI) que não existe em `src/llama_cpp` — linha de
  desenvolvimento paralela, não integrada com `dds-contract`/orchestrator Rust. Fora de
  escopo desta correção; mencionado para registro.

**Ação tomada (decisão do usuário: portar a extensão WF-3 para `third_party` agora):**
1. Copiados byte-a-byte `OrchestratorV4.idl`, `.c` e `.h` de `src/llama_cpp/dds/v4/idl/`
   para `third_party/llama.cpp_dds/dds/v4/idl/` (verificado com `cmp` — idênticos).
2. `dds-contract/build.rs` repontado de `../../../llama_cpp/...` para
   `../../../../third_party/llama.cpp_dds/...` (`OrchestratorDDS.idl` e `OrchestratorV4.idl`).
3. Validação: `CYCLONEDDS_STATIC=1 cargo check -p dds-contract --features dds` — ver
   `OPTIMIZATION_REPORT.md` para o resultado.

**Risco residual (não resolvido nesta sessão, fora do escopo do que foi pedido):**
- `dds_v4_bridge.cpp` em `third_party/llama.cpp_dds` não foi atualizado para emitir/ler os
  10 novos tipos nem os campos novos de Task/TaskOutput — ele continua funcionalmente
  equivalente ao que já fazia (publica Task/AgentState/TaskOutput/SystemMetric básicos). Se
  o objetivo for ter o C++ participando dos tópicos novos (Context/ToolCall/Security/QoS.*),
  isso é trabalho adicional não coberto aqui.
- `src/llama_cpp/` continua existindo e não foi arquivado — se for removido no futuro sem
  que alguém note que `dds-contract/build.rs` já foi repontado, não há mais risco (já aponta
  para `third_party`), mas os dois diretórios seguem duplicados até uma decisão de
  arquivamento explícita do usuário.
- `SystemMetric.value` mudou de `double` para `float` na árvore `third_party` — qualquer
  outro consumidor C++/Python desse campo (fora de `dds_v4_bridge.cpp`, não auditado nesta
  sessão) deveria ser revisado antes de considerar isso totalmente fechado.

---

## 1. Mapa arquitetural

### 1.1 Crates e responsabilidades

| Crate | Papel | LOC (arquivos) | Testes (`#[test]`+`#[tokio::test]`) | `benches/` |
|---|---|---|---:|:--:|
| `orch-common` | Tipos/métricas compartilhados (`TaskStatus`, `FuzzyMetrics`, instrumentação) | 1 arquivo, 247 linhas | não verificado nesta sessão | não |
| `qos-nfcm` | Decisão de QoS (NFCM + 4 baselines) | 11 arquivos em `src/` | 27 (10 baselines + 5 lib + 2 membership + 4 fcm_parity + 6 zadeh_parity) | não (harness `examples/five_arms.rs`) |
| `dds-contract` | Tipos DDS gerados do IDL + perfis QoS | 3 arquivos (`lib.rs`, `qos.rs`, `roles.rs`) | 20 (conforme `specs/000-dds-contract/REPORT.md`) | não |
| `dds-dataspace` | Camada DDS: 18 tópicos, caches, writer pool, monitor | 7 arquivos | 13 (conforme `specs/200-dds-dataspace/REPORT.md`) | não (bench de propagação citado no REPORT foi manual, não Criterion) |
| `agent` | Claim loop + ponte para llama-server | 7 arquivos | não recontado nesta sessão (3 + E2E conforme REPORT) | não |
| `orchestrator` | HTTP API (axum), scheduler, reaper, control loop NFCM | 6 arquivos | não recontado nesta sessão (6+ conforme REPORT) | não |
| `llm-gateway` | Roteamento LLM, pool, cache, rate-limit | 2 arquivos | não recontado nesta sessão | não |
| `client` | Submissão de tasks | 1 arquivo | não recontado nesta sessão | não |
| `spike-interop` | Harness de interop/benchmark standalone | `bin/`, `lib.rs`, `benches/roundtrip.rs`, `scripts/` | — | **sim** (`benches/roundtrip.rs`, único Criterion do workspace) |
| `policy-engine` | Avaliação de políticas MCP | 7 arquivos | **39** (verificado) |não |
| `context-store` | Contexto conversacional via DDS | 5 arquivos | **17** (verificado) | não |
| `mcp-gateway` | Gateway de tool calls MCP | 7 arquivos + `tools/` | **11** (verificado) | não |
| `observability` | Coleta QoS/trace/métricas | 9 arquivos | **15** (verificado) | não |
| `benchmarks` | Gerador de carga + driver de workload | 8 arquivos | **21** (18 unit + 3 loopback DDS, verificado) | não |

Os contadores marcados "verificado" foram obtidos por `grep -c '#\[test\]\|#\[tokio::test'`
nesta sessão e batem exatamente com os números publicados em `PLANO_EXECUCAO.md`/`README.md`
— **nenhuma divergência encontrada** nos números de teste auditados.

### 1.2 Fluxo de uma requisição (hot path)

```
1. Cliente HTTP → orchestrator::main::submit_task (axum handler, POST /api/v1/chat/completions)
2. orchestrator constrói Task, publica via OrchestratorDds::write_task
   (dds-dataspace: DataWriter<Task>.write(&task) — CÓPIA, não zero-copy; strength=10/cliente)
3. agent::AgentDds claim loop consome stream_tasks() (WaitSet/take_aiter, dedicado por stream)
   → claim.rs elegibilidade → write_task(claimed) para tomar ownership (Exclusive, strength=100)
   → confirmação via read_task_mesh() (estado arbitrado pelo RHC, não pelo cache local)
4. agent::DdsEngine publica LLMInferenceRequest, chama llama-server via DDS,
   recebe LLMInferenceResult/chunks correlacionados por request_id
5. agent publica TaskOutput (chunks) via WriterPool::submit (canal crossbeam + N workers
   escrevendo no MESMO DataWriter<TaskOutput> compartilhado — write() com cópia)
6. orchestrator consome stream_task_outputs(), consolida via select! entre streams
7. Resposta HTTP de volta ao cliente
```

**Pontos de lock/clone/(de)serialização no caminho crítico:**

| Hop | Lock | Clone | (De)serialização |
|---|---|---|---|
| orchestrator: scheduler push | `tokio::sync::RwLock<Scheduler>` (`crates/orchestrator/src/dds.rs:138`) | `task.clone()` antes do push | — |
| dds-dataspace: write | — (DataWriter thread-safe, sem lock explícito) | — | XCDR serialize dentro do `write()` (cópia C, não medida aqui) |
| agent: claim | — | `task.clone()` (`claim.rs:70`), depois mais 3–4 clones do mesmo Task ao longo de `dds.rs` (linhas 87–193: `claimed_task.clone()`, `running = task.clone()`, `final_task = task.clone()`) | `messages_json`/`model_name` (`String`) clonados por campo em cada re-clone |
| agent: cache read | `DashMap` shard lock (interno, curto) | `(*a).clone()` ao sair do cache (`dds-dataspace/src/lib.rs:1023,1037,1085`) — desreferencia `Arc<Task>` e clona o `Task` de novo | — |
| orchestrator: control loop NFCM | — | — | `serde_json::from_str` em `qos_routing.rs` (3 unwrap) por período de decisão |

**Observação central:** o cache (`dds-dataspace/src/cache.rs`) já guarda `Arc<Task>`
(`pub type ArcTask = Arc<Task>`), mas todo consumidor que atravessa a API pública (`read_task`,
`stream_tasks` do trait `DataSpaceApi`, e o próprio `agent`) imediatamente desreferencia e
clona o `Task` de novo (`(*a).clone()`) em vez de propagar o `Arc`. O ganho do `Arc` no cache é
anulado na borda da API — ver Plano, P1.

### 1.3 Control plane vs data plane

- **Control plane:** `orchestrator` (scheduler, reaper, registry, loop NFCM), `policy-engine`,
  `observability` — decisões e agregação, não streaming linha-a-linha.
- **Data plane:** `dds-dataspace` (todas as 18 filas de tópicos), `agent` (claim + streaming de
  chunks), `llm-gateway` (roteamento por request).
- **Hot path (por requisição):** submit → write Task → claim → DdsEngine → chunks TaskOutput →
  consolidação. **Cold path:** registro/heartbeat de agente (5s), reaper (staleness scan),
  controle NFCM (periódico), policy/mcp/context/observability (eventos esporádicos).
- **CPU vs I/O:** inferência em si roda no `llama-server` C++ (fora do processo Rust); o
  processo Rust é quase todo I/O-bound (espera DDS/HTTP) com trechos de CPU pontuais
  (NFCM/Zadeh/FCM decide, JSON parse, serialização XCDR feita pela lib C).

### 1.4 Tópicos DDS — produtores/consumidores (18 tópicos, `dds-contract::topics`)

| Tópico | Produtores (write) | Consumidores (stream/subscribe) |
|---|---|---|
| `Tasks` | `client`, `orchestrator` (strength 10), `agent` (reassume, strength 100), `benchmarks::driver` | `agent` (claim loop), `orchestrator` (consolidação/registro) |
| `AgentRegistry` | `agent` (heartbeat) | `orchestrator` (registry/reaper) |
| `TaskOutput` | `agent` (via `WriterPool`) | `orchestrator`, `client` |
| `SystemMetrics` | não localizado uso ativo nesta sessão (tipo gerado, sem writer/reader identificado no grep) | — |
| `LLM.InferenceRequest/Result/Error` | `agent::DdsEngine` (request) / `llama-server` C++ (result/error) | `agent::DdsEngine` |
| `Context.Snapshot`/`Context.Update` | não localizado producer explícito nesta sessão (contrato pronto, WF-8 entregou o consumidor) | `context-store::service` (`stream_context_snapshots`/`stream_context_updates`) |
| `ToolCall.Request` | `mcp-gateway::service` (`write_tool_call`, 3 sites) | `mcp-gateway::service` (`subscribe_tool_calls`) — mesmo crate publica e consome (ciclo de confirmação/estado) |
| `Execution.Trace` | não localizado producer explícito nesta sessão | `observability` (`stream_execution_traces`) |
| `Security.PolicySnapshot`/`Update` | `policy-engine::service` (`write_security_snapshot`) | não localizado consumidor explícito nesta sessão |
| `QoS.Metric`/`Violation`/`Discovery` | não localizado producer explícito nesta sessão (provável orchestrator/monitor) | `observability::qos_collector` (`stream_qos_metrics`, `stream_qos_violations`, `stream_discovery_events`) |

**Limitação documentada:** o mapeamento de produtores para `SystemMetrics`,
`Context.Snapshot/Update`, `Execution.Trace`, `QoS.*` foi feito por grep direcionado
(`stream_*`/`write_*`) e pode estar incompleto — alguns writers podem estar atrás de
abstrações (`self.data_space.write_*` chamado de um `main.rs`/binário não escaneado
diretamente). Não foi possível compilar um grafo 100% completo sem uma leitura exaustiva de
todos os `main.rs`, o que ficou fora do orçamento de tempo desta sessão.

---

## 2. Achados da auditoria estática (por categoria)

### 2.1 Concorrência assíncrona

- ✅ **Nenhum `unsafe` em nenhuma das 14 crates do workspace** (`grep -rn unsafe crates/*/src`
  = 0 ocorrências). Todo `unsafe` do sistema está confinado à crate vendorizada `cyclonedds`
  (fronteira FFI), consistente com a convenção do `AGENTS.md`.
- ✅ **Nenhum uso de `spawn_blocking`, `block_on` ou canal `unbounded`** em nenhuma crate —
  não há chamada bloqueante óbvia rodando dentro do runtime Tokio sem isolamento, nem canal
  sem backpressure.
- ✅ **Locks corretamente tipados por contexto:** `orchestrator` e `agent` usam
  `tokio::sync::RwLock` (async-aware) para estado compartilhado atravessado por `.await`
  (`crates/orchestrator/src/dds.rs:43,138`). Os `std::sync::Mutex` encontrados
  (`qos-nfcm/src/baselines.rs`, `qos-nfcm/src/fcm.rs`, `llm-gateway/src/lib.rs:100`,
  `observability/src/sink.rs:57`) protegem estado puramente síncrono e local (contadores de
  decisão, `Instant` de rate-limit, buffer de eventos) — nenhum caso observado de
  `std::sync::Mutex` retido através de um `.await` nas amostras verificadas. `tokio::sync::Mutex`
  é usado corretamente para I/O assíncrono (`context-store/src/local.rs` — journal de arquivo;
  `benchmarks/src/driver.rs:116` — writer JSONL).
- ⚠️ **`WriterPool` (T-204/T-305) é MPMC real, mas com writer único compartilhado por tópico:**
  N threads dedicadas (`spawn().name("dds-writer-{i}")`) consomem de um `crossbeam-channel`
  bounded e escrevem no **mesmo** `DataWriter<T>` por tipo (`writer_pool.rs:115-126`) —
  `DataWriter` é thread-safe para `write` concorrente, então isso é seguro, mas todo o
  paralelismo do pool converge para 3 handles de escrita (Task/Agent/Output); não há
  paralelismo adicional por sharding de writer. **Isso refuta a suspeita antiga do
  `MIGRATION_GAP_ANALYSIS.md` de "writer dedicado por worker sem MPMC real"** — o desenho
  atual É um MPMC genuíno (múltiplos produtores via canal, múltiplos workers), só que o
  paralelismo de escrita real depende de `DataWriter::write` ser lock-free/thread-safe no lado C
  (não verificado com profiling nesta sessão).
- ℹ️ **Streams por tópico usam um `WaitSet` dedicado por chamada** (`dds-dataspace/src/lib.rs`,
  17 blocos `take_aiter()` em `stream_tasks`/`stream_agent_states`/.../`stream_discovery_events`).
  O `ACTION_PLAN_DDS_IMPLEMENTATION.md` (T-617) propôs um `WaitSet` compartilhado com
  `ReadCondition` por tópico para evitar 1 thread de blocking-pool por stream; **isso NÃO foi
  implementado** — `PLANO_EXECUCAO.md` (WF-4/WF-8) não menciona T-617 como concluída. Com 18
  tópicos e potencialmente múltiplos assinantes (agent+orchestrator+context-store+mcp-gateway+
  observability+policy-engine), o número de streams simultâneos ativos em produção pode
  aproximar-se de 1 WaitSet/thread por consumidor-tópico — não medido, risco a confirmar sob
  carga real com vários processos ativos ao mesmo tempo (spike-interop roda com poucos
  streams por vez, não estressa este eixo).

### 2.2 Memória e alocações

- ⚠️ **`Arc<Task>` no cache não se propaga para a API pública** (ver §1.2): `dds-dataspace/src/lib.rs:1023,1037,1085`
  fazem `(*a).clone()` ao sair de `read_task`/`stream_tasks`/`stream_task_outputs` da
  implementação do trait `DataSpaceApi` — ou seja, todo consumidor via essa API (incluindo
  `agent` e `orchestrator`) recebe um `Task`/`TaskOutput` **owned** (clonado), não o `Arc`
  barato. O `agent` então clona esse `Task` mais 3–4 vezes ao longo do processamento
  (`claim.rs:70`, `dds.rs:91,130,180`) — cada clone copia as `String`s de `messages_json`
  (potencialmente grande, é o prompt) e `model_name`. **Confirma parcialmente o achado do
  `MIGRATION_GAP_ANALYSIS.md`** ("Task clonada em vez de `Arc<Task>`") — o tipo `Arc<Task>`
  existe e é usado internamente no cache, mas o desenho da API do trait não o expõe.
- ⚠️ **Zero-copy loans (`write_loan`/`request_loan`/`take_loan`) NÃO são usados em nenhum
  writer do workspace.** `grep -rn "write_loan\|take_loan\|request_loan"` só encontra uma
  menção em comentário de documentação (`dds-dataspace/src/lib.rs:11`, tabela "antes/depois"
  descrevendo a intenção) — **nenhuma chamada real**. Todos os 18 escritores usam `.write(&x)`
  (cópia), incluindo o hot path de streaming (`TaskOutput`, potencialmente milhares de chunks
  por sessão de inferência). **Confirma o achado do `MIGRATION_GAP_ANALYSIS.md`/`ACTION_PLAN`
  (T-616) — não implementado**, apesar de `PLANO_EXECUCAO.md` (WF-4) declarar a camada DDS
  "completa". O ganho medido de 88,7k tasks/s já supera os orçamentos por larga margem, então
  a urgência é baixa, mas é o item de maior ROI teórico ainda em aberto para o streaming de
  chunks (que é per-token, potencialmente o maior volume de samples do sistema).
- ⚠️ **`ahash` é dependência declarada mas não usada.** `ahash` está em
  `[workspace.dependencies]` (`Cargo.toml:48`) e no `Cargo.toml` de `orch-common`, mas
  `grep -rn "ahash\|AHasher"` não encontra nenhum uso em código — todos os ~20 sites de
  `DashMap::new()` no workspace usam o hasher padrão (SipHash). **Confirma o achado do
  `MIGRATION_GAP_ANALYSIS.md`** — dependência morta / otimização não aplicada. Baixo risco,
  ganho não medido (SipHash é mais lento que `ahash` para chaves curtas como `task_id`/`agent_id`
  em mapas de alta frequência, mas o overhead relativo não foi perfilado nesta sessão).
- ✅ **`orch-common` cresceu além do estado "mínimo" registrado no `MIGRATION_GAP_ANALYSIS.md`**
  (datado de 2026-07-15): hoje tem 247 linhas (o gap analysis registrava ~51 LOC e cobrança de
  enums faltantes). Não foi possível, no orçamento desta sessão, verificar se todos os enums
  citados (`TaskPriority`, `ModelSpecialization`, etc.) foram de fato adicionados — recomenda-se
  revisão pontual antes de assumir esse item como resolvido.

### 2.3 Algoritmos e estruturas

- Não foi identificada nenhuma busca linear óbvia no hot path de seleção de agente dentro do
  orçamento desta sessão (o `Scheduler` usa `BinaryHeap`, conforme `specs/300-control-plane/REPORT.md`).
  Uma leitura linha-a-linha de `selector`/`scheduler` não foi feita — **recomenda-se
  verificação dedicada antes de qualquer alteração de algoritmo** (fora do escopo desta
  auditoria por tempo).
- **CORREÇÃO (pós-verificação manual, 2026-07-20):** o achado original aqui e o item P2
  correspondente no `OPTIMIZATION_PLAN.md` estavam **incorretos na direção e na localização**.
  `orchestrator/src/qos_routing.rs::build_routing_profile()` faz 3 `serde_json::to_string`
  independentes (SERIALIZAÇÃO, não parse) para montar `allowed_agent_prefixes_json`,
  `weights_json` e `explanation_json` — cada um já com `.unwrap_or_else(|_| "...".to_string())`
  como fallback seguro (não há `unwrap()` cru nem panic possível no caminho de produção). Os
  `unwrap()` que o item P2 citava (linhas 150/160/161/173) estão **inteiramente dentro de
  `#[cfg(test)] mod tests`**, verificando o round-trip do próprio output da função — código de
  teste correto e idiomático, não um risco de produção. Além disso, `QoSRoutingProfile` **não é
  consumido em runtime por nenhum outro crate nesta base de código** (confirmado por busca em
  `observability`, `dds-dataspace`; o tópico é só publicado, o consumidor é "fora do escopo desta
  migração" segundo o próprio doc comment do módulo) — logo não existe hoje um caminho onde JSON
  malformado de um peer DDS não confiável chegaria a ser desserializado por este código. Item
  retirado do plano de remediação; risco real = nenhum. A única oportunidade remanescente é
  cosmética (P3, opcional): combinar os 3 `to_string()` em um único `serde_json::to_string` de
  uma struct/`json!({...})` composta, para 1 alocação em vez de 3 — não prioritário.

### 2.4 DDS

- **QoS profiles** (`dds-dataspace/src/qos.rs`) são descritos no REPORT como espelho fiel do
  `dds_data_space.py` — não reauditado linha-a-linha nesta sessão; aceito como estado
  reportado.
- **`latency_budget` não é mutável em runtime** neste CycloneDDS (achado de WF-6,
  `dds_set_qos` → `OUT_OF_MEMORY`) — limitação de infraestrutura, não de código Rust; já
  documentada e contornada (o knob fica fora do "hot set", herdado do perfil de criação).
- **`WaitSet` por stream** — ver §2.1 (não resolvido, T-617 pendente).
- Nenhuma evidência de `ContentFilteredTopic`/CFT sendo usado (a crate suporta CFT
  writer-side; `specs/010-interop-spike/REPORT.md` já registrava isso como "avaliar depois" —
  ainda não avaliado).

### 2.5 Observabilidade

- `observability/src/sink.rs` usa `std::sync::Mutex<Vec<ObservabilityEvent>>` como buffer —
  simples e correto para o volume esperado, mas um buffer que só cresce
  (`Vec` sem cap observado no grep) é um risco de memória sob alta cardinalidade de eventos se
  não houver dreno/flush periódico — não verificado se há flush automático (fora do
  orçamento desta sessão; **recomenda-se checagem pontual**, não incluída no plano P0–P3 abaixo
  por falta de evidência suficiente).
- `tracing`/`tracing-subscriber` são dependências do workspace; não foi auditado o custo de
  spans no hot path (criação de spans por chunk de `TaskOutput`, por exemplo) — **gap de
  auditoria**, não uma afirmação de problema.

---

## 3. Resultado dos comandos de validação

| Comando | Resultado | Tempo | Observação |
|---|---|---|---|
| `cargo fmt --all -- --check` | ✅ PASSOU (sem diffs) | <5s | — |
| `cargo check --workspace --all-targets` | ✅ PASSOU | 4 min 33 s | Builda a `cyclonedds` C real (via `cyclonedds-build`) mesmo **sem** `--features dds` explícito — `--all-targets` inclui `spike-interop/benches/roundtrip.rs`, que depende da crate real sem feature-gate. Isso quebra a premissa do README/AGENTS.md de que "`cargo check` fica rápido, sem builda o C" — **só é verdade para `cargo check --workspace` sem `--all-targets`**. Achado documentado, não corrigido nesta sessão (ver Plano, P2). |
| `cargo test --workspace` | ✅ **PASSOU** — **196 testes passados, 0 falhas** em 64 binários de teste (incluindo doc-tests vazios por crate) | ~6 min (reaproveitou o cache do `check`) | Nenhuma linha `FAILED` no log; todas as 64 ocorrências de `test result:` são `ok`. |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ **PASSOU** — build `Finished` sem nenhum warning/erro emitido | ~2 min (após liberar o lock do `test`) | `-D warnings` faz qualquer warning falhar o build; terminou limpo. |
| `CYCLONEDDS_STATIC=1 cargo build -p agent --features dds` | **Não executado separadamente nesta sessão** — o `cargo check --all-targets` e o `cargo test --workspace` já acionaram e reaproveitaram o build C completo da `cyclonedds` (ver acima); um build isolado de `agent --features dds` seria redundante dado o cache já quente. | — | Gap de auditoria menor — recomenda-se rodar isoladamente antes de qualquer deploy, só para confirmar o binário de produção. |
| `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1` | **Não executado nesta sessão** (o `cargo test --workspace` acima já roda sem a feature `dds`, cobrindo os testes não feature-gated). | — | Gap de auditoria — os testes DDS-real de `dds-dataspace` (os 13 do REPORT WF-4) não foram re-executados nesta sessão; recomenda-se rodar antes de aceitar qualquer alteração nesse crate. |

### Limitações e incidentes desta sessão (transparência)

1. Uma primeira tentativa de rodar `cargo check --workspace --all-targets` em background via
   `nohup ... &` (fora do mecanismo de background da própria ferramenta) continuou viva e
   **segurou o lock do `CARGO_TARGET_DIR`** por vários minutos depois que a checagem manual de
   processos (`ps`) indicava (incorretamente) que nada estava rodando — uma segunda tentativa de
   `cargo check` ficou bloqueada esperando esse lock. O processo original foi localizado via
   `ps -eo pid,ppid,cmd` (PID real do `cargo`) e a tentativa redundante foi encerrada; o processo
   original terminou com sucesso. **Lição registrada:** neste ambiente, processos em background
   iniciados fora do parâmetro dedicado da ferramenta podem sobreviver ao encerramento aparente
   do shell e não aparecer de forma confiável em buscas superficiais de processo — usar sempre
   o mecanismo de background nativo da ferramenta e, se precisar depurar, buscar pelo binário
   exato (`cargo`/`rustc`) com `ps -eo pid,ppid,cmd`, não só `grep` solto.
2. `cargo test --workspace` e `cargo clippy --workspace --all-targets -- -D warnings` foram
   disparados em paralelo e competiram pelo mesmo lock de `CARGO_TARGET_DIR` (o clippy ficou
   "Blocking waiting for file lock" por um tempo) — ambos terminaram e **passaram limpos**
   antes do fechamento deste documento (196 testes ok / 0 falhas; clippy sem warnings). Rodar
   os dois em paralelo contra o mesmo target dir só serializa a parte que compete pelo lock;
   não economiza tempo real — para próximas sessões, preferir rodá-los em sequência ou aceitar
   a serialização como esperada.
3. Listagens de diretório em `dds-contract/src`, `observability/src`, `benchmarks/src`
   apresentaram travamentos intermitentes em sessões anteriores deste mesmo host (mount
   SMB/CIFS) — nesta sessão, com `timeout 20` e no máximo uma repetição, todas as três
   resolveram com sucesso. Não houve necessidade de fallback para leitura direta de arquivo.

---

## 4. Verificação dos itens do `MIGRATION_GAP_ANALYSIS.md` (2026-07-15) contra o código atual

| Item do gap analysis | Status verificado agora | Evidência |
|---|---|---|
| Zero-copy loans disponíveis mas não usados | **CONFIRMADO — ainda não implementado** | 0 chamadas a `write_loan`/`take_loan`/`request_loan`; todos os writers usam `.write(&x)` |
| `Task` clonado em vez de `Arc<Task>` | **PARCIALMENTE CONFIRMADO** | Cache usa `Arc<Task>` internamente (`cache.rs:20`), mas a API pública do trait desreferencia e clona (`lib.rs:1023,1037,1085`); `agent` clona o `Task` mais 3–4× no processamento |
| `take_aiter()` cria WaitSet dedicado por stream (T-617 não feito) | **CONFIRMADO — ainda não implementado** | 17 blocos `take_aiter()` independentes em `dds-dataspace/src/lib.rs`; nenhuma menção a `WaitSet` compartilhado no código ou nos REPORTs de WF-4/WF-8 |
| `DashMap` não usa `ahash` | **CONFIRMADO — ainda não implementado** | 0 usos de `ahash`/`AHasher`; dependência declarada mas morta |
| `WriterPool` não é MPMC real (writer dedicado por worker) | **REFUTADO — foi corrigido** | `writer_pool.rs` usa canal `crossbeam` + N workers escrevendo no mesmo `DataWriter` compartilhado por tipo — é MPMC genuíno, não um writer por worker |
| `orch-common` "mínimo" (51 LOC) | **DESATUALIZADO — cresceu** | Hoje 247 linhas; conteúdo exato não reauditado linha-a-linha nesta sessão |

---

## 5. Oportunidades descartadas nesta rodada (e por quê)

- **Reescrever o `WriterPool` para usar `write_loan`:** de alto valor teórico, mas sem medição
  de alocação real (nenhum profiler de memória rodou nesta sessão — `heaptrack`/`DHAT` não
  foram executados por orçamento de tempo). Fica como P1 no plano, não implementado agora.
- **Trocar hasher do `DashMap` para `ahash`:** baixo risco, mas também sem medição de
  throughput antes/depois — vai para o plano como P2, não implementado agora (a etapa 6 do
  processo exige medição antes/depois antes de aceitar).
- **Investigar CFT (`ContentFilteredTopic`)**: mencionado em dois REPORTs como "avaliar
  depois" — sem uso atual de filtro SQL identificado no `dds_backend` Python nesta sessão;
  descartado por falta de evidência de necessidade real.
- **Perfilar com `perf`/`flamegraph`/`tokio-console`:** nenhuma ferramenta de profiling estava
  disponível/pré-configurada para rodar contra um sistema DDS ao vivo dentro do tempo desta
  sessão (exigiria subir `llama-server` + múltiplos agentes reais). Documentado como gap de
  reprodutibilidade — ver Plano, item sobre harness de benchmark.
