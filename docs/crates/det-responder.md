# `det-responder` — respondedor determinístico (não-produção)

> Crate: `src/rust/crates/det-responder/` · **Backend determinístico de benchmark
> (REQ/T-820-14) — NÃO é lógica de produção.**

## Propósito

Consome `LLM.InferenceRequest` via DDS e publica fixtures `LLMInferenceResult`.
Fiação: tópicos `LLM.*` com QoS de produção, fila limitada, N atendentes, log
JSONL. **O que NÃO é:** não chama LLM real, não faz inferência, não serve
tráfego; só EXP1/EXP3.

Derivação da fixture: `normalize(role:conteúdo)` + `sha256 hex[..16]` + texto
`[fixture stage={S} h={h} n={n}]`, chunking em 2 partes (mid-point UTF-8 safe).
Estágio via marcador explícito `PROMPT_VERSION:` na mensagem `system` (nunca
ordem de chegada).

## Como rodar

```bash
cd src/rust
cargo run -p det-responder -- --domain 78 --delay-ms 5 --capacity 4 --queue 64 --log resp.jsonl [--model M]
# Defaults: domain=78, delay_ms=5, capacity=4, queue=64, log=det-responder.jsonl. Sem features: DDS sempre ligado.
```

Fluxo: reader `LLM.Request` → `admit` (try_send; fila cheia = erro 429
observável) → N workers `handle` → 2 chunks em `LLM.Result` (`seq 0/1`,
`finish 0/1`, `tokens 0,0/10,5`) ou `LLM.Error` (400/429/500) + linha JSONL.
SIGINT fecha admissão e drena com graça de 30 s; contadores finais
`ok/err/rejected_full` no stderr. `capacity==0` rejeitado. Sem `--help`.

Contrato externo = DDS (request → 2 results ou error) + log JSONL (`seq,
request_id, task_id, agent_id, model, stage, family, hash, n_msgs,
arrival/started/finished_ns, delay_ms_config, outcome, error?`) + stderr
(`READY`/`EXIT` com contadores). Módulos: `endpoint.rs` (publish_error/admit/
handle), `fixture.rs` (parse/normalize/hash/split; `STAGE_TABLE` seq/fork × A/B/C).

## Testes

```bash
cargo test -p det-responder   # 7 unitários em fixture.rs (marcador, ordem irrelevante, hash == Python, split UTF-8)
```

## Limites

- **Não é LLM**: tokens fake (`10/5`); `model_used` ecoa o pedido sem validar;
  medir qualidade/latência real com ele mede o harness, não o modelo.
- Fila cheia → **429 imediato** no Rust vs espera 30 s no stub Python —
  caudas p99+ cross-backend não comparáveis sem isolar (paridade confirmada só em
  conteúdo/hash/chunking).
- **Relógios mistos**: JSONL em `Instant` relativo ao t0 vs driver em epoch ns
  (`emitted_at_ns` em wall) — juntar artefatos sem reancorar gera latências absurdas.
- `admit` clona cada request; workers disputam `rx` sob um `Mutex`; `publish_error`
  ignora falha de write; log faz `flush` por linha; `--delay-ms` não modela
  TTFT/streaming real.
- Teste `hash_igual_ao_backend_python` depende de `python3` no PATH e escreve
  `/tmp/fx-norm.txt`; `endpoint.rs` sem testes.
- Sem auth, sem DDS Security, sem rate-limit além da fila — nunca expor o
  domínio desses harnesses fora da bancada.
