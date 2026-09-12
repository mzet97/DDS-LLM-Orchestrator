# src/rust — todos os projetos: função e como usar

> Guia autocontido para uma IA sem nenhum contexto prévio do repositório.
> Verificado contra o código em 2026-09-10. Todos os caminhos abaixo são
> **absolutos** (corte o prefixo `/home/mzet/projetos/tese/` se o checkout
> estiver em outro lugar). Bins exigem `--features dds` no build.

## -1. Onde você está

- Raiz do repositório: `/home/mzet/projetos/tese/`
- Este workspace: `/home/mzet/projetos/tese/src/rust/` (este arquivo:
  `/home/mzet/projetos/tese/src/rust/CONTEXTO.md`)
- Vizinhos relevantes: `/home/mzet/projetos/tese/src/orchestrator/` (Python
  legado de referência), `/home/mzet/projetos/tese/src/llama_cpp/` (servidor
  C++ de inferência), `/home/mzet/projetos/tese/third_party/` (dependências
  vendorizadas, incl. CycloneDDS), `/home/mzet/projetos/tese/benchmarks/
  orchestration/` (harness Python de comparação, prompts, resultados).
- Regra de ouro: **neste workspace, o transporte padrão é DDS pub/sub**
  (Eclipse CycloneDDS, mesmo wire format XTypes/XCDR do C++ e do Python).
  HTTP/gRPC são auxiliares ou legado — nunca o caminho quente.

## 0. Glossário mínimo (leia isto primeiro)

- **Domínio DDS** (`--dds-domain N`): fronteira de isolamento; participantes
  de domínios diferentes **não se veem**. Use um domínio livre (ex. 78);
  nunca assuma que mudar só o cliente isola o teste — todos os participantes
  próprios (driver, agente, responder) precisam do mesmo domínio.
- **Tópico**: canal nomeado tipado (ex. `Tasks`, `LLM.InferenceRequest`).
- **Claim**: o agente lê tasks `PENDING` e toma uma para si (atribuição
  distribuída por `ownership strength`; sem despachante central, sem DAG).
- **Ciclo de vida da Task**: `PENDING(0)` → `ASSIGNED` → `RUNNING` →
  `DONE(3)`/`FAILED(4)`. Só `submit()` do `client` decide o fim (DONE +
  algum chunk `is_final`).
- **`task_id`/`request_id`**: UUID v4 gerado por submit; o agente publica a
  inferência com `request_id = task_id` — é **o** vínculo de correlação
  ponta a ponta (nunca confie só no conteúdo do texto).
- **Fixture**: resposta determinística de benchmark
  `[fixture stage={A,B,C} h={sha256[:16]} n={n_msgs}]` (não é LLM real).
- **TransientLocal**: durabilidade que entrega o **histórico recente** a
  leitores novos — releituras de tail após (re)iniciar um participante são
  **comportamento normal e idempotente**, não bug, não exactly-once.
- **`PROMPT_VERSION:`**: marcador dentro da mensagem `system` (ex.
  `seq_reviewer_v1`) que o `det-responder` usa para derivar a etapa. Nunca
  decidir etapa por ordem de chegada.

## 0.1. Base comum (vale para todos os comandos abaixo)

```bash
export CYCLONEDDS_URI='<CycloneDDS><Domain/></CycloneDDS>'  # sem isto, o default mira a NIC stale enp4s0 e o participant falha (-1)
export CARGO_TARGET_DIR=/tmp/seu-target   # opcional; default é src/rust/target (use isolado se outros usam a máquina)
cd /home/mzet/projetos/tese/src/rust
```

Toolchain stable 1.95 (`rust-toolchain.toml`), edition 2021, lints
`warnings = deny` (rust + clippy). CycloneDDS 3.0.0 via path
`third_party/cyclonedds-rust` (detalhes e armadilhas na §16; antes de
compilar pela 1ª vez, leia aquela seção — o submodule `vendor/cyclonedds`
pode precisar de `git submodule update --init vendor/cyclonedds` dentro de
`third_party/cyclonedds-rust`, e a `libddsc.a` de um build cmake).

---

## 1. `agent` — worker que executa tasks via DDS

**Função.** Reivindica tasks `PENDING` (claim distribuído por strength),
executa inferência e publica `TaskOutput`. É o único executor do caminho
quente (não há DAG no runtime).

**Arquivos.** `src/main.rs` (CLI clap), `lib.rs`, `dds.rs` (`AgentDds`,
claim loop, `process_and_publish`), `claim.rs`, `engine.rs` (trait `Engine`
+ `InferRequest`), `engine_dds.rs` (`DdsEngine`: publica
`LLM.InferenceRequest` sempre com `stream:true`, readers de Result/Error
por chamada + settle 250ms, filtra por `request_id`, termina no `is_final`),
`engine_http.rs`, `heartbeat.rs`.

**Como usar.**

```bash
cargo build -p agent --features dds
/tmp/tgoal-target/debug/agent --agent-id agent-01 --dds-domain 78 \
  --engine dds --slots 8 --model qwen3.5-0.8b --specialization text \
  --llama-url http://localhost:8082 --provider-constraint local-only
```

Flags: `--engine dds|http|mock`. Com `--engine mock`, sem DDS real (só teste).

## 2. `client` — quem submete tasks e recebe resultados

**Função.** Biblioteca `DdsClientDds` + 3 bins. `submit()` escreve a Task,
coleta `TaskOutput`s do próprio `task_id`, espera `DONE + is_final`
(qualquer ordem), ordena por `seq_num` e concatena. `create_task` gera
`task_id` UUID v4 (`temperature` 0.7, `max_tokens` 256).

**Como usar.**

```bash
cargo build -p client --features dds
# 1 task avulsa (saúde do caminho):
/tmp/tgoal-target/debug/submit-one 78 "qwen3.5-0.8b" '[{"role":"user","content":"oi"}]'
# stdout: {"task_id","content","success","latency_ms","tokens_prompt","tokens_completion"}

# carga N tasks (p50/p95/p99), sequencial ou --concurrent:
/tmp/tgoal-target/debug/e2e-bench -- 78 20 [--concurrent]

# driver de workload (cadeia/barreira/serial em Rust, sem Python):
/tmp/tgoal-target/debug/wf-run --domain 78 --workload seq_chain_v1 \
  --entry "texto" --prompts-dir benchmarks/orchestration/prompts \
  --model qwen3.5-0.8b --workflow-id w1 --out record.json
# workloads: seq_chain_v1 | fork_join_v1 | fork_join_serial_v1
# stdout: WF_RECORD {...}; arquivo = mesmo JSON (stages com task_id/content/expects/latências)
```

`wf/assembly.rs` replica `assembly.py` do Python; `wf/record.rs` monta o
registro. `submit_http` existe mas é legado — não usar no caminho DDS.

## 3. `det-responder` — backend determinístico DDS (benchmark)

**Função.** Assina `LLM.InferenceRequest` reais, deriva a etapa por
correspondência explícita `PROMPT_VERSION:` (`fixture.rs:STAGE_TABLE`, 6
marcadores; ordem de chegada nunca decide), publica a fixture
`[fixture stage=h=…=n=]` em 2 chunks (`seq_num` 0,1) com a mesma fórmula do
backend HTTP Python, e publica `LLMInferenceError` (400/429/500) para o
inesperado. Capacidade/fila limitadas; log JSONL por requisição.

**Como usar.**

```bash
cargo build -p det-responder   # já puxa feature dds das deps
/tmp/tgoal-target/debug/det-responder --domain 78 --delay-ms 5 \
  --capacity 8 --queue 64 [--model qwen3.5-0.8b] --log resp.jsonl
# stderr: READY ... / STOP ok=.. err=.. rejected_full=..
```

Não é lógica de produção. Alternativa C++ foi avaliada e descartada
(tipos gerados reutilizados aqui).

## 4. `orchestrator` — supervisor/monitor (fora do caminho quente)

**Função.** Ingestão HTTP (axum) + registry/selector + decisão de QoS
(NFCM). Monitora e recupera; **não** despacha etapas do workload.
Recuperação pelo monitor sem evidência própria (B-02 aberto).

**Como usar.**

```bash
cargo build -p orchestrator --features dds
/tmp/tgoal-target/debug/orchestrator --port 8085 --dds-domain 42 \
  [--qos-manager ...] [--qos-profile ...] [--fuzzy-routing]
```

**Arquivos.** `main.rs` (app + parse manual), `lib.rs`, `dds.rs`,
`state_machine.rs`, `qos_monitor.rs`, `qos_routing.rs`.

## 5. `dds-contract` — tipos e tópicos canônicos (base de tudo)

**Função.** Gera os tipos Rust das IDLs via `cyclonedds-build` (`build.rs`)
e publica as constantes de tópicos/QoS. Toda crate DDS depende dela.

**Arquivos.** `lib.rs` (`topics::`, `profiles::ALL`, `generated::`,
`dds::`, `typenames::`), `qos.rs`, `roles.rs`, `build.rs`, `tests/`.

**Como usar (como lib).**

```rust
use dds_contract::topics;                       // Topics, LLM.InferenceRequest, ...
use dds_contract::generated::orchestrator::{LLMInferenceRequest, LLMInferenceResult};
use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput, AgentState};
```

Regras: IDLs-fonte em `third_party/llama.cpp_dds/dds/idl/OrchestratorDDS.idl`
(LLM.*, keyless) e `…/v4/idl/OrchestratorV4.idl` (Task/TaskOutput/…); tipos
`LLM.*` **keyless por requisito** (REQ-003, sem `@key`); `Task.status`:
0=PENDING, 3=DONE, 4=FAILED; correlação sempre por `request_id == task_id`.

## 6. `dds-dataspace` — DataSpace, QoS e streams

**Função.** Camada DDS: `DataSpace::new(domain_id, ownership_strength)`
com readers/writers, `SharedWaitSet`, caches lock-free (`TopicCaches`,
`ArcTask`, `ArcLLMResult`, `llm_results_of(request_id)`) e perfis QoS.
`in_memory.rs` = `InMemoryDataSpace` só para ablação (nunca caminho quente).

**Arquivos.** `lib.rs` (API), `api.rs` (`DataSpaceError`), `dispatch.rs`,
`cache.rs`, `monitor.rs`, `writer_pool.rs`, `qos.rs` (profiles), `in_memory.rs`.

**Como usar (como lib).**

```rust
let ds = DataSpace::new(78, 100)?;
ds.write_task_sync(&task)?;
let tasks: Vec<Task> = ds.take_tasks_sync()?;
let mut s = ds.stream_task_outputs();   // stream_* por tópico (Tasks, LLM.*, …)
let qos = dds_dataspace::qos::profiles::llm_result()?;  // KeepLast(256), Reliable, TransientLocal
```

**Quirk:** tópicos `LLM.*`/`Tasks` são TransientLocal — leitor novo relê o
tail do histórico (replays idempotentes; consumidores filtram por
`request_id`/`task_id`; sem exactly-once).

## 7. `llm-gateway` — roteamento a provedores (lib, sem bin)

**Função.** Roteia requisições a provedores local (C++) / cloud, com
failover multi-worker (`failover.rs`). Porte do `llm_gateway/` Python.
Usada pelo `orchestrator` (dependência no `Cargo.toml` dele).

**Como usar:** como dependência de outro crate (`llm-gateway = { path =
"../llm-gateway" }`); não tem executável próprio.

## 8. `context-store` — serviço de contexto conversacional

**Função.** Persiste contexto (`Context.Snapshot`/`Context.Update`) com
journal local JSONL e espelha no DDS. Porte do `context_store/` Python.

**Como usar.**

```bash
cargo build -p context-store --features dds
/tmp/tgoal-target/debug/context-store --dds-domain 78 \
  --data-file context_store.jsonl --log-level INFO
# Uso: context-store [--dds-domain N] [--data-file PATH] [--log-level LEVEL]
```

**Arquivos.** `main.rs` (parse manual + `--help`), `lib.rs`, `local.rs`,
`service.rs`, `store.rs`.

## 9. `policy-engine` — motor de políticas de segurança

**Função.** Avalia regras (`policies.json` ao lado do `Cargo.toml`),
cache local com TTL, distribui snapshots/deltas
(`Security.PolicySnapshot/Update`) via DDS. Sem política carregada, o
default é permissivo (igual ao gateway Python). Porte do `policy_engine/`.

**Como usar.**

```bash
cargo build -p policy-engine --features dds
/tmp/tgoal-target/debug/policy-engine --dds-domain 78 \
  --policy-file crates/policy-engine/policies.json \
  --republish-interval-secs 30 --log-level info
```

**Arquivos.** `main.rs` (`app::run`), `lib.rs`, `service.rs`
(`PolicyEngineService`), `engine.rs`, `rules.rs`, `cache.rs`, `error.rs`.
Tem `tests/` e `policies.json` no dir do crate.

## 10. `mcp-gateway` — ferramentas via `ToolCall.Request`

**Função.** Expõe ferramentas aos agentes e executa com governança de
política: publica/consome `ToolCall.Request` (o contrato **atualiza a mesma
instância** — não inventar tópico `ToolCall.Response`). Tools em
`src/tools/`: `filesystem.rs`, `external.rs` (github/web/database/ci-cd no
descritivo do crate). Default permissivo; `--max-security-level 0..3`
restringe.

**Como usar.**

```bash
cargo build -p mcp-gateway --features dds
/tmp/tgoal-target/debug/mcp-gateway --dds-domain 78 \
  --filesystem-root /tmp/sandbox [--sandbox-dir DIR] \
  [--max-security-level 0..3]
# default: domínio 0, raiz /tmp/sandbox, política permissiva
```

**Arquivos.** `main.rs` (parse + `--help`), `lib.rs`, `service.rs`,
`handler.rs`, `dds.rs`, `policy.rs`, `error.rs`, `tools/{mod,filesystem,external}.rs`.

## 11. `observability` — telemetria e coletores

**Função.** Sink JSONL + coletores de QoS (`qos_collector.rs`,
`qos_store.rs`), traces (`trace_collector.rs`, `Execution.Trace`) e
trackers (`trackers.rs`: tokens/RTT/custo). Porte de `observability/`,
`qos_collector/`, `trace_collector/`, `metrics/` do Python.

**Como usar.**

```bash
cargo build -p observability --features dds
/tmp/tgoal-target/debug/observability-collector --dds-domain 78 --output-dir ./obs-out
```

**Arquivos.** `main.rs` (app + parse), `lib.rs`, `dds.rs`, `events.rs`,
`sink.rs`, `qos_collector.rs`, `qos_store.rs`, `trace_collector.rs`, `trackers.rs`.

## 12. `benchmarks` — gerador de carga E1–E5/OP1–OP4

**Função.** Roda um cenário contra a malha DDS e grava JSONL (a análise
estatística continua no Python). Porte de `benchmarks/` Python.

**Como usar.**

```bash
cargo build -p benchmarks --features dds
/tmp/tgoal-target/debug/dds-bench --scenario E4 --domain 78 \
  --duration 60 --seed 42 --out ./bench-out --model qwen3.5-0.8b \
  --arm baseline --workers 10 --timeout-ms 120000
# cenários: E1..E5, OP1..OP4 (default E4)
```

**Arquivos.** `main.rs` (app + parse), `lib.rs`, `driver.rs`
(`DriverConfig`), `generator.rs`, `scenarios.rs`, `regimes.rs`, `metrics.rs`, `rng.rs`.

## 13. `spike-interop` — sondas de interoperabilidade (Fase 0b)

**Função.** Bins de diagnóstico Rust↔Python↔C++ via DDS + `build.rs` que
força link estático do CycloneDDS pré-compilado (aceita `CYCLONEDDS_BUILD`).

**Como usar** (bins em `src/bin/`, ex. `cargo run -p spike-interop --bin pub_task --features dds`):
`pub_task`/`sub_task` (round-trip de Task), `pub_stream`/`sub_stream`,
`llm_client`, `rtt_bench`, `diag_knobs` (domínio 110 fixo), `dump_ops`
(inspeciona XTypes ops do `Task`), `repro_layout`.

## 14. `qos-nfcm` e `orch-common` (libs puras, sem bin/serviço)

- **`qos-nfcm`**: NFCM neuro-fuzzy para seleção adaptativa de QoS
  (`fcm.rs`, `zadeh.rs` — Zadeh Extension Principle, `nfcm.rs`,
  `membership.rs`, `decider.rs` — `QoSMetrics`/`QoSDecision`/`StaticDecider`,
  `trainer.rs`, `baselines.rs`, `dataset.rs`, `stability.rs`, `utility.rs`).
  Uso: `StaticDecider::new(profile)` / avaliar `NfcmResult`
  (`explain_text(&r)`). Artigo em `artigo_fuzzy_extension_qos/`.
- **`orch-common`** (`lib.rs` único): tipos, config, métricas e
  instrumentação compartilhados por todas as crates.

## 15. Fluxo quente ponta a ponta (benchmark)

```
wf-run --workload W --entry E
 → DdsClientDds::submit(Task PENDING, task_id=UUID)
 → [Tasks] agent claim (strength) → DdsEngine publica LLM.InferenceRequest
    (request_id=task_id, stream=true)
 → det-responder (ou llama-server --enable-dds): LLM.InferenceResult* (chunks)
 → agent: TaskOutput por chunk → Task DONE(3)/FAILED(4)
 → wf-run concatena por seq_num → record JSON
```

Correlação por `request_id == task_id` em todo o caminho; `agent_id` do log
do responder por junção de `task_id`.

## 16. Dependência DDS e gates (não reverter)

- Paths: `third_party/cyclonedds-rust/<crate>` (sem segmento duplo),
  `/home/…` (sem `/var/home`). Submodule `vendor/cyclonedds` em `5131ff3`;
  `libddsc.a` +fPIC; `.cargo/config.toml` linka `-l:libddsc.a` global.
- `CYCLONEDDS_URI` mínimo obrigatório (NIC stale `enp4s0`); domínio 78 no
  benchmark; `cargo build/test/clippy` com `--features dds` e
  `CARGO_TARGET_DIR` isolado; `cargo fmt --check`.
- Anti-padrões: sem exactly-once/fencing e2e, sem MCP ativo afirmável, sem
  vitória sobre MAF; HTTP/gRPC auxiliares; `retry_count` ≠ fencing token.

## 17. Verificação ponta a ponta em 5 minutos (copie e cole)

```bash
export CYCLONEDDS_URI='<CycloneDDS><Domain/></CycloneDDS>'
export CARGO_TARGET_DIR=/tmp/seu-target
cd /home/mzet/projetos/tese/src/rust
cargo build -p agent -p client -p det-responder --features dds   # ~1-9 min na 1ª vez (compila o C do CycloneDDS)
T=$CARGO_TARGET_DIR/debug
setsid -f $T/det-responder --domain 78 --delay-ms 5 --log /tmp/resp.jsonl </dev/null >>/tmp/resp.log 2>&1
sleep 2; grep -q READY /tmp/resp.log && echo "responder ok"
setsid -f $T/agent --agent-id a1 --dds-domain 78 --engine dds --slots 8 </dev/null >>/tmp/agent.log 2>&1
sleep 4; grep -q "claim loop iniciado" /tmp/agent.log && echo "agente ok"
$T/submit-one 78 "qwen3.5-0.8b" '[{"role":"user","content":"oi"}]' --timeout-ms 60000
# esperado: {"content":"[fixture stage=...]", ...,"success":true}  (stage vazio→erro 400; use prompt com PROMPT_VERSION: p/ fixture)
# log do responder (/tmp/resp.jsonl): 1 linha por requisição {request_id,task_id,stage,hash,outcome}
```

Encerre só os seus (anote os PIDs via `ps -eo pid,args | grep ...` e `kill`
exatamente eles; nunca `pkill` genérico nem mexa em processos do domínio 42).

## 18. Quero X → mexa em Y (índice por tarefa)

| Quero… | Mexa em… | Não mexa em… |
|---|---|---|
| Mudar regra de claim/associação agente-task | `crates/agent/src/{claim,dds}.rs` | tópicos/QoS (quebra C++/Python) |
| Mudar o que o agente publica em `LLM.*` | `crates/agent/src/engine_dds.rs` | `det-responder` junto sem atualizar `STAGE_TABLE` |
| Mudar fixture/comportamento do backend bench | `crates/det-responder/src/{fixture,endpoint}.rs` (+ testes em `fixture.rs`) | lógica de produção (`agent/`, `client/src/lib.rs`) |
| Novo workload/ordenação | `crates/client/src/bin/wf_run.rs` + `wf/` | `submit()` (semântica de conclusão) |
| Novo tópico ou campo IDL | **IDL primeiro** (`third_party/llama.cpp_dds/dds/…`), depois `dds-contract/build.rs` + espelhos C++/Python | só o `.rs` gerado (é build artifact) |
| Nova política de segurança | `crates/policy-engine/policies.json` + `rules.rs` | default permissivo sem documentar |
| Nova ferramenta de agente | `crates/mcp-gateway/src/tools/` + `handler.rs` | nome de tópico novo (reutilize `ToolCall.Request`) |
| Ajustar QoS de um tópico | `crates/dds-dataspace/src/qos.rs` + teste de compatibilidade leitor↔escritor | `llm_result` KeepLast(256) sem refazer DDS-QOS-004 |
| Adicionar cenário de carga | `crates/benchmarks/src/scenarios.rs` | análise estatística (fica no Python) |
| Diagnóstico de interoperabilidade | `crates/spike-interop/src/bin/` (novo bin) | `build.rs` (link estático sensível — só via `CYCLONEDDS_BUILD`) |

## 19. Não tocar (quebra silenciosa em outra linguagem/processo)

- `third_party/cyclonedds-rust` com `git checkout/reset/clean` (tem
  modificações locais) ou `submodule update --remote` (fixo em `5131ff3`).
- `.cargo/config.toml` (link estático global) e `spike-interop/build.rs`.
- Domínio 42 e processos alheios (`orchestrator --port 8085`, `llama-server`
  GPU): observar com `ps`, nunca matar/reiniciar.
- `~/.bashrc` (env sempre inline por comando) e `src/rust/target` alheio
  (use `CARGO_TARGET_DIR` próprio).
