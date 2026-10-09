# `client` — cliente DDS + drivers de workload

> Crate: `src/rust/crates/client/` · Lib + 3 binários (exigem `dds`).

## Propósito

Submeter tasks e coletar resultados com **UM participante DDS servindo N tasks
async** (resolve o deadlock de ~20 clientes do Python, cada um com seu
participante+GIL). Inclui montagem canônica de mensagens A→B→C e drivers de
workload/benchmark.

## Binários

| Binário | Uso |
|---|---|
| `submit-one` | `<domain> <model> <messages_json> [--timeout-ms N]` → JSON em stdout |
| `wf-run` | `--domain 78 --workload {seq_chain_v1\|fork_join_v1\|fork_join_serial_v1} --entry ... [--prompts-dir DIR] [--model M] [--workflow-id ID] [--out F] [--timeout-ms N] [--target-agent T]` |
| `e2e-bench` | `<domain> <n> [--concurrent]` → `RESULT_JSON {...}` (p50/p95/p99) |

```bash
cd src/rust
cargo run -p client --features dds --bin submit-one -- 0 qwen3.5-0.8b '[{"role":"user","content":"oi"}]'
```

`wf-run`: `seq_chain_v1` = A→B→C; `fork_join_v1`/`fork_join_serial_v1` = barreira;
`--target-agent` para EXP4. `e2e-bench --concurrent` compartilha um `DdsClientDds`.

## API pública (lib)

- `DdsClient::{new, create_task, submit_http}`, `ClientConfig` (default domain 0,
  timeout 120 s), `ClientError` (Timeout/TaskFailed/DdsError/EventLagged/
  EventChannelClosed/RuntimeUnavailable/EventPumpInit/IncompleteStream/HttpStatus),
  `TaskResult`, `seq_gap` (validação de contiguidade `seq_num`, compila sem `dds`).
- `wf_assembly::{build_messages, load_prompt, SEQ_PROMPTS, ANALYSIS, REVIEW,
  CORRECTNESS, SECURITY}` — prompts congelados byte-idênticos ao Python
  (`include_str!` de `assets/`; réplica de `assembly.py`).
- Com `dds`: `dds_impl::DdsClientDds::{new, dataspace, submit, submit_stream}` —
  UM participante + 2 pumps broadcast cap 4096; `new` exige runtime Tokio;
  `submit` finaliza só com DONE **e** chunk `is_final`, com gap-check antes de
  `success:true`; `submit_stream` entrega `IncompleteStream` como último item.
- `wf/record.rs`: `RunMeta`/`StageRec`/`emit` — só escreve em sucesso
  (ausência de arquivo = não-concluído).

## Testes

```bash
cd src/rust
cargo test -p client
CYCLONEDDS_STATIC=1 cargo test -p client --features dds -- --test-threads=1  # 50+ concorrentes, dom 102
```

`agent` é dev-dep (com `dds`). Domínios de teste fixos.

## Limites

- `submit_http` é unidirecional: retorna `task_id`, sem polling/stream do resultado.
- Segurança DDS opt-in; `DdsClientDds` não expõe construtor com `SecurityConfig`.
- Prompts congelados em `assets/`: mudar o canônico Python exige ressincronização manual.
- Relógios mistos no record: `latency_ms` (Instant) vs `started/finished_ms`
  (parede) não são subtraíveis.
