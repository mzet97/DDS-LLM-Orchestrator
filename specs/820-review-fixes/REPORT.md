# REPORT — specs/820-review-fixes (2026-10-04)

**Fonte:** `CODE_REVIEW_SRC_RUST_2026-10-04.md` (raiz do workspace). **Resultado: 21/21 tasks aplicadas; gates verdes** — `fmt` ✅, `clippy --workspace --all-targets -- -D warnings` ✅ (0 warnings), `cargo test --workspace --no-fail-fast` **402 passed / 0 failed** (era 358 — +44 testes), incluindo os E2E dds-gated (`t820_failover_reatribuicao_e_reclaim_por_outro_agente`, `agent_claim_process_complete_e2e`, `dds_loopback`).

## P0 — corrigidos
| Task | Correção | Teste |
|---|---|---|
| T-820-01 | `[workspace.lints.clippy] warnings="deny"` removido (lint `clippy::warnings` não existe — E0602; quebrava o gate `-D warnings` do CI) | gates |
| T-820-03 | **Failover reconstruído.** (1) reaper publica via `write_task_without_ownership` (strength cliente 10 — antes strength 200 congelava a instância); (2) `state_machine::reassign` com teto de retries + **renovação de `created_at_ns`** (P0-2); (3) reaper de progresso para ASSIGNED/RUNNING estagnadas (30 s/60 s); (4) **republisher** `pending_reassignment` — re-publica por tick até o mesh refletir o PENDING (TTL 60 s). **Descoberta de semântica DDS** (ver abaixo) | `t820_failover_*` E2E, `test_reassign_renova_created_at_ns`, `test_reassign_max_retries_fails` |
| T-820-04 | `TraceCollector::flush` drain-and-write (sem duplicação); QoS persistida no sink por ingest; SIGINT gracioso com flush final | `two_flushes_do_not_duplicate…`, `ingest_emits_sink_event_per_ingest` |

## Descoberta de semântica DDS (T-820-03) — registrar na dissertação
O fix do failover exigiu entender o RHC do CycloneDDS (`dds_rhc_default.c`):
1. **Write/dispose menores que o dono são rejeitados** (`inst_accepts_sample`): o PENDING em strength 10 contra o dono 100 (ASSIGNED do agente morto) é invisível.
2. **Dispose também passa pelo filtro E transfere o ownership** (`update_inst_have_wr_iid` no dispose aceito): dispose(200) deixa a instância *disposed e dona do 200* — beco sem saída pior.
3. **Ownership só é liberado pela destruição do writer do dono** (`relinquish_ownership` via `unregister_wr`): morte real → lease do DDSI; delete gracioso → imediato.
Design final: sem dispose; reaper publica em 10 e o **republisher** repete até o mesh confirmar. **Implicação experimental:** EXP4 (tempo de recuperação) inclui o lease do DDSI — medir e documentar; o "agente morto" do teste deve destruir o participante (`drop`, não `mem::forget` — o esquecimento mantém o writer vivo e o ownership nunca libera).

## P1/P2 — resumo por área
- **agent (T-820-09):** `request_id` por tentativa (`{task_id}#{retry_count}` — evita replay TL(256)); settle 250 ms uma vez por processo; `finish_reason` canônico via `FinishReason`; `is_claimed` point-check (sem clone por amostra); VRAM via `spawn_blocking`; engines validam status/`messages_json` (erros tipados); FAILED best-effort em caminhos de erro pós-claim; `--engine` estrito; EMA atômica + health derivado de falhas consecutivas.
- **orchestrator (T-820-10):** cache feeders com supervisão (recriação com backoff 1→30 s — `/sync` não morre mais para sempre); `Scheduler::push` agora descarta a de MENOR prioridade (o bug era pior que o descrito: descartava a de MAIOR).
- **client (T-820-11):** gap-check de `seq_num` (`ClientError::IncompleteStream` — fim do truncamento silencioso); status HTTP validado; CLI estrita; `record` async.
- **llm-gateway (T-820-12):** cache key via FNV-1a sem `agent_id` (hits cross-agent); eviction FIFO real; half-open com sonda única (`compare_exchange`).
- **dds-dataspace (T-820-05/06/07/20):** readers de stream com QoS do tópico (fim do BestEffort/KL1 no caminho crítico) + setup eager + backoff de take; admissão de cache `(Arc,bool)` em todos os upserts/streams; `not_alive_writers_net`; WriterPool com métricas honestas e `drain_and_shutdown -> (submitted, completed, failed)`; mock com regressão/caps/dedup (EXP1a mais fiel); `ShutdownFlag` encerra streams no Drop do DataSpace; FNV-1a determinístico no roteamento do pool; shutdown doc corrigido.
- **orch-common/dds-contract (T-820-02):** `ToolCallStatus` canon 0..5 + `is_terminal*`; `OnlineKnobs` trata `latency_budget=0` como não configurado; gate de paridade mock↔IDL (`tests/mock_parity.rs`).
- **qos-nfcm (T-820-08):** guarda de finitude no NFCM (NaN → Balanced/converged=false — paridade com FCM/Zadeh); cooldown sem off-by-one; faixa [0,1] documentada.
- **benchmarks (T-820-13):** stream de status ÚNICO por corrida (fim do O(N²) por request); `closed_loop` itera a grade de concorrência do cenário (E2/OP1 reais); SIGINT drena+flush; warmup avaliado no instante de ENVIO e fora da duração medida; arquivo com run_id; CLI estrita; `one_stream` sem is_final → erro; tokens nunca fabricados.
- **serviços (T-820-14..18):** det-responder (paridade documentada, SIGINT com drenagem); context-store (`expire_ttl` por ticker + tombstone no journal); policy-engine (regressão de versão rejeitada; republish não reverte deltas; SIGINT); mcp-gateway (overload publica FAILED sem claimar — fim do descarte silencioso; no-lease documentado); studio-node (`spawn_blocking` p/ systemctl; journal com rollback; ledger limitado).
- **studio (T-820-19):** I/O bloqueante fora da thread de UI (thread+mpsc+poll em inference/dispatch/observe/catalog/services).

## Desvios do plano
1. `dispose_task` chegou a ser implementado e **foi removido** após a descoberta (2) — sem uso no design final.
2. `runner_up` no fallback do NFCM = `0.0` (campo é `f64`, não `Option`).
3. `Scheduler::push`: corrigido o comportamento (não só o warn).
4. `drain_and_shutdown(self)` agora retorna as estatísticas finais (contadores só são estáveis pós-join — corrige corrida em teste).

## Pendências deliberadas (fora do escopo — mudança coordenada de contrato)
`tokens_prompt` no IDL; `emitted_at_ns` no C++; backlog keyless `LLM.*` (REQ-003); alinhamento `priority`/`ModelSpecialization` no IDL (pré-congelamento da dissertação).
