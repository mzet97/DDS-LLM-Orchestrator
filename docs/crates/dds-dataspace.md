# `dds-dataspace` — camada DDS de coordenação

> Crate: `src/rust/crates/dds-dataspace/` · Substitui `dds_backend/` (Python, ~3,4k LOC) ·
> 2º alvo da migração Rust (após o agente).

## Propósito

Subir 1 participant + os **19 tópicos canônicos** com QoS que casa com a malha
Python, readers/writers por tópico, caches concorrentes e streams por evento —
eliminando os gargalos mapeados do backend Python:

| Gargalo Python | Solução nesta crate |
|---|---|
| Poll loop 20 ms + churn por amostra | WaitSet compartilhado + streams async; 1 thread de espera por `DataSpace` |
| Alocação por amostra | Loans zero-copy (`write_output_loan`) no hot path |
| Thread única de escrita | N workers + canal MPMC `crossbeam-channel` |
| Caches (dict + RLock global) | `DashMap` sharded com `ahash` |
| Corrida estrutural C1 | Ownership por papel + tipos imutáveis (`Arc<Task>`) |
| Liveliness por polling | Listener nativo `on_liveliness_changed` |

Sem a feature `dds`, compila um `DataSpace` stub (só guarda `domain_id` /
`ownership_strength`) e o mock `InMemoryDataSpace` continua disponível.

## API pública: trait `DataSpaceApi` (~40 métodos)

Famílias write/read/subscribe por tópico: Tasks, Agents, TaskOutput, telemetria
(`SystemMetric`/`ServerStatus`), LLM (request/result/error), Context, ToolCall,
ExecutionTrace, Security, QoS, `shutdown`. Pontos de desenho:

- **Sem write-through**: o cache é alimentado APENAS pelas streams (visão do mesh);
  read-after-write é eventualmente consistente (~ms).
- **Confirmação de ownership** lê o RHC arbitrado (`read_task_mesh` /
  `read_tool_call_mesh`), nunca o cache — o cache refletiria o próprio echo
  (risco de execução dupla, T-203).
- `write_task_without_ownership` usa writer de strength CLIENTE=10 para
  reaper/reatribuição não congelar a instância (T-820-03).
- `ToolCall` evolui **in-place na mesma instância** (sem tópico de resposta).
- `write_tool_call_without_ownership` tem default que retorna erro
  (`WriteFailed("não implementado")`); `read_tool_call` é obrigatório, sem
  default silencioso.

## Componentes

- **`DataSpace`** (`new` / `new_with_profile` / `new_with_profile_and_security`):
  participant/publisher/subscriber, 19 tópicos + writers/readers, caches,
  `SharedWaitSet`. Drop ordenado (filhos antes dos pais). `write_studio_node_presence`
  (heartbeat síncrono 5 s), `apply_tasks_knobs` (só transport priority /
  ownership strength a quente — latency budget retorna OUT_OF_MEMORY neste
  CycloneDDS), `new_writer_pool`, `caches()`.
- **Pool de writers de `Tasks`**: 64 slots para papel agente (1 p/ demais),
  strength sorteada por processo (quebra desempate determinístico por GUID) e
  roteamento determinístico cross-processo por `fnv1a64(task_id)` (mesmo slot em
  todos os agentes — exigido pela arbitragem Exclusive).
- **19 streams `stream_*` por evento**: reader dedicado por chamada + registro no
  WaitSet; setup EAGER com o QoS do tópico (nunca defaults — corrige perda
  silenciosa em tópicos Volatile); só entrega amostra aceita pelo cache;
  backoff 100 ms→5 s; término gracioso via `ShutdownFlag`.
- **`TopicCaches`**: upserts devolvem admissão explícita; `retry_count` vence
  sempre, senão rejeita status menor e `assigned_agent` esvaziado (last-write-wins
  reflete o vencedor da arbitragem; sem monotonia de timestamp, deliberado).
  Caps: 2048 tasks, 256 chunks/chave, 256 presenças, TTL 30 s p/ terminais sob pressão.
- **`WriterPool`**: canal bounded MPMC, falha rápida com backpressure,
  `submit_with_ack` para o chunk final (RUST-PROTO-005), `drain_and_shutdown`
  devolve (submitted, completed, failed), `write_output_loan` zero-copy.
- **`QosMonitor`**: listeners nativos (liveliness de agentes, deadline de outputs)
  → `broadcast(256)` de `QosEvent` + contadores.
- **`InMemoryDataSpace`**: mock com `DashMap` + `broadcast(1024)`/tópico.
  Divergências documentadas: consistência write→read imediata, sem arbitragem
  Exclusive (last-write-wins), sem RHC, sem TransientLocal p/ late joiners,
  entrega síncrona. Replica caps e dedup por (`task_id`, `seq_num`).
  Contract tests A/B rodam a mesma bateria nos dois backends.

## QoS por tópico (espelha o Python, medido via SEDP; todos Reliable 10 s salvo nota)

`tasks` Rel+TL+KL50+Exclusive+liveliness 10 s+tprio 8; `task_output` KL64+deadline
10 s; `agent_registry` Shared+ManualByTopic 10 s+deadline 30 s+KL1; `system_metrics`/
`server_status` BE+Volatile+KL1; `qos_metric` Rel 5 s+TL+KL100+tprio 7; `qos_violation`
Rel 5 s+TL+KL1000+tprio 8; `qos_discovery` Rel 5 s+Volatile+KL50+tprio 6;
`execution_trace` TL+KL256+Exclusive+tprio 5; `llm` request/error TL+KL10;
`llm_result` TL+KL256 (+durability_service — perfil separado após perda medida de
108/128 com KL10, Gate C2); `context_snapshot` TL+KL1+Exclusive; `context_update`
TL+KL10+Exclusive (Volatile→TL p/ late joiner, T-850-03/D2); `tool_call` TL+KL10+
Exclusive (+ variante com strength, T-890-06); `security_*` Rel 5 s+TL+KL1+
Exclusive+tprio 9; `qos_routing` Rel 5 s+TL+KL1+tprio 9; `studio_node_presence` =
perfil AgentRegistry (T-890).

## Binários, examples e benches

- Bin `security_echo` (requer `security`): helper do smoke de DDS Security (T-813);
  `publisher|subscriber|intruder <domain> <security_dir>`; subscriber imprime
  `RECEIVED <server_id>` e sai 0, senão 1 após 30 s.
- Examples (sondas): `exp1a` (ablação in-memory×DDS, dissertação §3.7.3),
  `domain_probe`, `list_node_presence`, `stream_probe`, `tool_call_probe`.
- Benches (criterion): `cache_hasher` (ahash×SipHash, sem DDS), `write_loan`
  (requer `dds`, domínio 63).

## Features, env e testes

Features: `default=[]`; `dds` (runtime real); `security` (+ Security + bin).
`CYCLONEDDS_URI`: consumida pela lib CycloneDDS nativa (não lida aqui).
`CYCLONEDDS_STATIC=1` no build/test com `dds`.

```bash
cd src/rust
cargo test -p dds-dataspace                                     # mock + unit
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1
cargo test -p dds-dataspace --features dds,security --test security_smoke
cargo bench -p dds-dataspace --bench cache_hasher
```

Suíte `tests/`: `contract` (A/B mock+real), `lifecycle` (T-302), `llm_result_backlog`
(Gate C2), `monitor` (T-306), `qc_e2e`, `security_smoke` (T-813), `shared_waitset`
(T-617), `streams` (T-304), `t808_*`, `write_loan` (≥1000 chunks, 0 gaps),
`writer_pool` (T-305), `cache`.

## Limites

- Stub sem-`dds` não implementa `DataSpaceApi` — código genérico usa `InMemoryDataSpace`.
- Sem produtor nativo Rust de `DiscoveryEvent` (M3); produção via monitor Python
  ou escrita direta.
- `WriteRequest::Task` sem chamador em produção (pool usado p/ Agent/Output).
- Examples DDS além de `stream_probe` presumem `--features dds` sem gate declarado.
- `take_loan` da doc-raiz refere-se a `write_output_loan` (loan de escrita);
  não há loan de leitura com esse nome.
