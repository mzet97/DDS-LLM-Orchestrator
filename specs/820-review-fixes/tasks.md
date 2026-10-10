# specs/820-review-fixes — Correções do code review de 2026-10-04

Fonte: `CODE_REVIEW_SRC_RUST_2026-10-04.md` (raiz do workspace). Todos os commits desta fase usam `[T-820-XX]`.
Regra: nenhum commit sem `cargo test` + `clippy -D warnings` + `fmt --check` verdes; testes de paridade quando aplicável.

| Task | Severidade | Escopo | Estado |
|---|---|---|---|
| T-820-01 | P0-5 | Cargo.toml: remover lint inválido `clippy::warnings` (E0602) | [x] |
| T-820-02 | P1 | orch-common: `ToolCallStatus` alinhado ao canon 0..5 + teste de paridade | [x] |
| T-820-03 | P0-1/P0-2/P2-1 | orchestrator+dds-dataspace: reaper publica com strength ≤ agente (`write_task_as`), usa `state_machine::reassign` (max_retries), renova `created_at_ns`; reaper de ASSIGNED/RUNNING sem progresso; teste E2E de failover | [x] |
| T-820-04 | P0-3/P0-4 | observability: flush incremental de traces (drain), persistência de QoS no sink, handler de SIGINT | [x] |
| T-820-05 | P1/P2 | dds-dataspace: readers de stream com QoS do tópico; setup eager para tópicos Volatile; `alive_writers_net` correto; shutdown doc + dedup clears; flag de shutdown p/ streams; backoff crescente em erro de take | [x] |
| T-820-06 | P2 | dds-dataspace: admissão de cache propagada a todos os upserts/streams (padrão Accepted/Rejected); cap de SystemMetric com chave composta; docs de cache/monitor; métricas do WriterPool; guard de pool vazio | [x] |
| T-820-07 | P1 | dds-dataspace: InMemoryDataSpace — filtro de regressão de status + caps + dedup de outputs + divergências documentadas | [x] |
| T-820-08 | P1 | qos-nfcm: guarda de finitude no NFCM (fallback Balanced); off-by-one do cooldown; doc de faixa das métricas | [x] |
| T-820-09 | P1/P2 | agent: `request_id` por tentativa; settle 250 ms uma vez por processo; `finish_reason="COMPLETION"`; `claimed_set` sem clone; VRAM via spawn_blocking; engine_http valida status/messages_json; FAILED em caminhos de erro pós-claim; `--engine` value_enum; heartbeat EMA/contadores | [x] |
| T-820-10 | P1/P2 | orchestrator: supervisão dos cache feeders (recriação com backoff); `finish_reason` canônico no /sync; Scheduler::push warn/doc | [x] |
| T-820-11 | P1/P2 | client: gap-check de seq_num; submit_http valida status; e2e_bench parse de CLI; record.rs relógio/status | [x] |
| T-820-12 | P2 | llm-gateway: cache key hashada + eviction FIFO real; circuit breaker half-open com sonda única | [x] |
| T-820-13 | P1/P2 | benchmarks: stream de status único por corrida (open/priority); closed_loop itera grade de concorrência; Ctrl+C drena+flush; warmup por instante de submissão; arquivo com run_id; parse de CLI estrito; one_stream defensivo; priority tokens None | [x] |
| T-820-14 | P2 | det-responder: divergências de paridade documentadas (saturação/estágio); drenagem em Ctrl+C; doc de timestamps | [x] |
| T-820-15 | P2 | context-store: ticker de expire_ttl no serviço; doc de retenção | [x] |
| T-820-16 | P2 | policy-engine: recusar regressão de versão; republish só se versão ≥; handler de sinal | [x] |
| T-820-17 | P2 | mcp-gateway: responder FAILED/overload quando fila cheia (não descartar sample); doc de lease do claim | [x] |
| T-820-18 | P2 | studio-node: spawn_blocking p/ systemctl/probe; journal append-first; retenção do ledger | [x] |
| T-820-19 | P2 | orchestrator-studio: thread+mpsc (padrão de models.rs) em inference/dispatch/observe/shared_catalog | [x] |
| T-820-20 | P3 | lote menor: hasher determinístico (FNV-1a) no roteamento do pool; magic numbers → consts; docs (dispatch/cache/OnlineKnobs); REQ-XXX ausentes | [x] |
| T-820-21 | — | Integração: gates full (fmt/clippy -D/testes), REPORT.md, notes.md | [x] |

**Adiamentos deliberados (contrato/três linguagens — mudança coordenada REQ-003, fora do escopo desta fase):**
- `tokens_prompt` propagado no IDL/TaskOutput (hoje 0 no sistema); `emitted_at_ns` no C++; backlog keyless `LLM.*`; alinhamento `priority`/`ModelSpecialization` no IDL (já exigido pela dissertação pré-congelamento).
