# `orchestrator` — control-plane DDS-first

> Crate: `src/rust/crates/orchestrator/` · Porte de `src/orchestrator/orchestrator/`
> (~2,0k LOC Python) · Binário `orchestrator`.

## Propósito

Control-plane: ingestão HTTP (axum), scheduler por prioridade, registry de
agentes, seleção por especialização, loop de controle NFCM/QoS, state machine e
failover em cascata (T-424). **Não é despachante obrigatório**: publica `Tasks`
PENDING e os agentes reivindicam (data-centric); o orquestrador é
monitor/supervisor de recuperação (reapers + monitor QoS).

Semântica **at-least-once**. `retry_count` não é fencing token; dedup só de
blocos/outputs. Ownership DDS Exclusive arbitra claims: cliente=10 < agente=100
< orquestrador=200.

> Não existe `TaskManager`/`AgentSelector`/`RegistryListener` como tipos em Rust
> (só menção como gap): as funções estão fatiadas em `publish_task`,
> `spawn_registry_monitor` + `reap_dead_agents`/`reap_stalled_tasks`,
> `check_task_deadlines`, `state_machine::*`, `select_agent`.

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p orchestrator --features dds -- --dds-domain 0
# HTTP em 127.0.0.1:8080 por padrão. Sem `dds`, o main só informa o build degradado.
```

Exemplo mínimo (loopback sem auth → identidade `local-trusted`):

```bash
curl -X POST localhost:8080/api/v1/chat/completions -H 'Content-Type: application/json' \
  -d '{"model":"qwen3.5-0.8b","messages":[{"role":"user","content":"oi"}]}'
```

## Fronteira HTTP

| Rota | Papel |
|---|---|
| `GET /health` | Pública: `{status, component, timestamp}` |
| `POST /api/v1/chat/completions` | Assíncrona → `{task_id, status:"pending", t_serialization_ns, t_transport_send_ns}` |
| `POST /api/v1/chat/completions/sync` | Bloqueia até DONE/FAILED ou 504 (poll 25 ms; T1–T6) |
| `GET /api/v1/agents` | Inventário `{count, agents[]}` |

Protegidas passam por `authorize_and_limit` (Bearer [REDACTED] semáforo).
Validação: model/messages obrigatórios, allowlist→403, `message_count`→422,
`message_bytes`→413; erros 401/403/413/422/429/504/500 com códigos estáveis.
Bind não-loopback exige `--http-expose` + auth + allowlist; credenciais
`client=token` (token ≥ 32 chars, modo unix sem `0o077`), comparação em tempo
constante sobre digest SHA-256.

## Componentes

- **`Scheduler`** (BinaryHeap max, teto 1024; cheio descarta MENOR prioridade;
  pop = maior prioridade/mais antiga). **Sem consumidor em produção** —
  `publish_task` deliberadamente não alimenta (custo sem leitor).
- **`AgentRegistry`** (DashMap+ahash; saudável = `health==2` + slot livre) e
  **`select_agent`** (health+slots+especialização via `can_serve` + prefixo
  `target_agent`; escolhe o menos ocupado).
- **`state_machine`**: PENDING→ASSIGNED; ASSIGNED→RUNNING/PENDING/FAILED;
  RUNNING→DONE/FAILED/PENDING; DONE/FAILED terminais; wire desconhecido →
  `UnknownStatus` (nunca assume PENDING). `assign`/`start_running`/`complete`
  (`finish_reason=COMPLETION`)/`fail`; `reassign` respeita `max_retries`
  (→FAILED `MAX_RETRIES_EXCEEDED`), limpa atribuição e **renova `created_at_ns`**
  para não nascer "velha" para a elegibilidade de 10 s.
- **`OrchestratorDds`**: DataSpace strength orquestrador + writer de tasks
  strength CLIENTE (10). `dispatch_task` (EXP1b `--dispatch-mode`: fixa
  `target_agent`, claim do agente permanece). `publish_task` com strength 10
  para os agentes vencerem a arbitragem.
- **Supervisão**: `spawn_cache_feeders` (streams com recriação backoff 1 s→30 s,
  T-820-10); `spawn_registry_monitor` (feeder + `last_seen` + reapers por tick);
  `reap_dead_agents` (heartbeat parado → `QoS.Violation(liveliness_lost)` +
  reassign das tasks do morto); `reap_stalled_tasks` (ASSIGNED >30 s / RUNNING
  >60 s voltam a PENDING mesmo com agente vivo — fecha falso-negativo de claim);
  `republishe_reatribuicoes_pendentes` (TTL 60 s, renova `created_at_ns`, T-880/EXP4).
- **Loop de controle** (`spawn_control_loop`): métricas do mesh → `decide_once`
  (decisor `--qos-manager`: `nfcm|static|zadeh|fcm|fcm-dhl`, default `nfcm`) →
  `StabilityController` (histerese/cooldown) → `apply_tasks_knobs`; trace
  `qos_decision` com perfil bruto+efetivo; `maybe_publish_routing_profile`
  (opt-in `--fuzzy-routing`, OFF por padrão, dedup por perfil).
- **Observabilidade**: `check_task_deadlines` (1ª vez por task → Violation;
  dedup com teto 10k), `publish_qos_metrics` + `spawn_qos_monitor` (1 `QoS.Metric`
  por contador + evicção de terminais 30 s).

## Flags e features

Flags: `--bind/--port`, `--http-expose`, `--http-auth-file`, `--http-model`
(repetível), `--http-body-bytes/message-count/message-bytes/max-tokens/
concurrent-requests/dds-wait-timeout-ms`, `--dds-domain`, `--dds-secure` (+
`--dds-security-dir` com 6 arquivos), `--qos-manager`, `--qos-profile`,
`--fuzzy-routing`, `--dispatch-mode`. Env lidos: `DDS_AGENT_AMD_PREFIX`,
`DDS_AGENT_RTX_PREFIX`, `RUST_LOG` (fallback `info`).
Features: `default=[]`, `dds`, `security`. `--dds-secure` sem feature
`security` → erro de boot.

## Testes

```bash
cd src/rust
cargo test -p orchestrator
CYCLONEDDS_STATIC=1 cargo test -p orchestrator --features dds -- --test-threads=1
```

Cobertura: `scheduler` (T-402/T-404 + regressão T-820-10), `control_loop` (T-405),
`qos_manager` (T-504, 5 modos), `fuzzy_routing`, `qos_monitor`, `reaper` (T-403),
state-machine (9), HTTP/auth/limites (13, T1–T6), `qos_routing` (6),
`qos_monitor` (3), `http_config` (2). Domínios DDS 100–105 isolados.

## Limites

- Sem `TaskManager.reap_expired`: deadline expirado só gera Violation, nunca muda estado.
- Sem `--enable-agent-selection` (modo despacho do Python); análogo é `--dispatch-mode`.
- 8 listeners QoS nativos + `check_reliability_gaps` + produtor `QoS.Discovery` não portados.
- `tokens_prompt/completion` sempre 0 no `/sync`; `model_required` sempre 0 no HTTP.
- Sem fencing fim-a-fim: outputs de tentativas distintas com mesmo (`task_id`,seq)
  são indistinguíveis. Falso-negativo de claim mitigado (reapers+TTL), não eliminado.
- Segurança DDS opt-in; sem `--dds-secure`, só aviso (local-only por padrão).
