# `agent` — worker de claim PENDING e inferência

> Crate: `src/rust/crates/agent/` · Primeiro alvo da migração (maior ROI) ·
> Substitui `src/orchestrator/agent/` (~2k LOC Python) · Binário `agent`.

## Propósito

Worker que reivindica tasks `PENDING` via DDS (claim otimista ASSIGNED +
confirmação de ownership por readback), executa inferência num `Engine`
(`dds`/`http`/`mock`) e publica `TaskOutput` em streaming + estados terminais.

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p agent --features dds -- \
  --agent-id agent-rust-01 --slots 8 --dds-domain 0
CYCLONEDDS_STATIC=1 cargo run -p agent --features dds -- --engine mock  # sem llama-server
cargo run -p agent                                                      # sem DDS: 1 task mock local
```

Flags: `--agent-id`, `--dds-domain`, `--dds-secure` (+`--dds-security-dir`),
`--slots` (8), `--model` (qwen3.5-0.8b), `--specialization <text|vision|
embedding|transcription>` (desconhecido → Text, silencioso), `--engine
<dds|http|mock>` (inválido → erro de boot), `--llama-url`, `--target-agent-prefix`
(EXP4), `--provider-constraint <local-only|cloud-only>` (`http`+cloud-only → erro).
Env: `RUST_LOG` (default `info`); `CYCLONEDDS_STATIC=1` no build DDS.

## Ciclo de claim e execução

1. **Ingestão**: drena `stream_tasks` + pré-filtro → canal bounded `4×slots` (mín 16).
2. **Elegibilidade** (`is_eligible`): `status==PENDING` + idade ≤ 10 s +
   especialização (`can_serve`) + prefixo `target_agent`.
3. **Claim**: marca local (`mark_claimed`) → write ASSIGNED (`assigned_agent` +
   `assigned_at_ns`) → sleep `CONFIRM_DELAY` 250 ms → `read_task_mesh` até
   `CONFIRM_TIMEOUT` 10 s (`assigned_agent==eu && status==ASSIGNED`).
   Perdeu → `unmark_claimed`; venceu → processa.
4. **Execução** (`process_and_publish`): ASSIGNED→RUNNING; `request_id={task}#{retry}`
   (não replaya TransientLocal); chunks → `WriterPool`; final via `submit_with_ack`;
   DONE só após ack (RUST-PROTO-005); `t_agent_queue_ns`/`t_inference_ns` locais;
   FAILED best-effort preservando ownership pós-claim.
5. **Heartbeat**: `AgentState` a cada 5 s (VRAM via `spawn_blocking` + `nvidia-smi`).

Dispatcher revalida no cache fresco e rejeita deadline vencido; capacidade
divergente → FAILED best-effort.

## Engines

| Engine | Papel |
|---|---|
| `DdsEngine` | Participant próprio, writer de request reusado, readers de result/error por stream; settle único 250 ms; publica `LLMInferenceRequest` stream=true; correlaciona por `request_id`; reader de Result KL256 |
| `HttpEngine` | Só URLs loopback http(s), sem credenciais/redirect; POST `{base}/v1/chat/completions` **não-stream** (1 chunk final); erros tipados com corpo truncado |
| `MockEngine` | Chunks `"{content}-{i:04}"` para testes |

`AgentStatus`: slots/completed/failed/EMA latência/VRAM atômicos; health DEGRADED
após 3 falhas. `SlotGuard` RAII.

## API pública (lib)

`Agent::{new, claim_config, status, claimed_set, is_claimed, mark_claimed,
unmark_claimed, process_task}`, `AgentConfig`, `claim::{ClaimConfig,
Specialization, is_eligible, is_eligible_with, claim_task, confirm_ownership}`,
`engine::{Engine, MockEngine, InferRequest, Chunk, EngineError, ProviderConstraint}`,
`engine_http::HttpEngine`, `heartbeat::{AgentStatus, SlotGuard, SlotUnavailable}`;
com `dds`: `dds::AgentDds::{new, new_with_security, agent, dataspace,
spawn_heartbeat, run}`, `engine_dds::DdsEngine`.

## Testes

```bash
cd src/rust
cargo test -p agent
CYCLONEDDS_STATIC=1 cargo test -p agent --features dds -- --test-threads=1
CYCLONEDDS_STATIC=1 cargo test -p agent --features dds --test engine_dds -- --test-threads=1
# engine_dds exige llama-server real --enable-dds (domínio 91, porta 8091)
```

Suíte: `agent_e2e` (dom 90), `engine_dds` (91), `engine_overhead`,
`engine`, `writer_reuse` (105) + unitários (slots, CLI).

## Limites

- `Agent::process_task` (path não-DDS) só loga chunks — o produtivo é `AgentDds`.
- `HttpEngine` não faz streaming; cloud via HTTP bloqueado por desenho.
- Segurança DDS opt-in (sem `--dds-secure`, aviso e sem auth/cripto).
- Especialização CLI desconhecida cai em Text sem erro; spec desconhecida no
  selector nunca casa — task pode ficar sem elegível silenciosamente.
