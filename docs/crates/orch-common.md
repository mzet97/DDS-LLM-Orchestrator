# `orch-common` — tipos compartilhados do runtime

> Crate: `src/rust/crates/orch-common/` · Arquivo único `src/lib.rs` (742 linhas) ·
> Sem dependência de DDS · Substitui `src/orchestrator/common/` (Python).

## Propósito

Biblioteca de tipos compartilhados do runtime Rust:

- **Views tipadas** (`#[repr(i32)]` + `TryFrom<i32>` / `From<T> for i32`) sobre os
  campos `long` crus do wire DDS, com erro único `UnknownEnumValue`.
- **Vetor de métricas** do decisor fuzzy (`FuzzyMetrics`, 8 entradas).
- **Instrumentação concorrente** sem GIL (`LatencySpan`, `RttTracker`, `ErrorCounter`).

## API pública (raiz do crate)

| Item | Papel |
|---|---|
| `TaskStatus` (Pending=0..Failed=4) | Status de task; `is_terminal()` / `is_terminal_i32()` (terminais 3\|4) |
| `TaskPriority` (**Low=1, Normal=5, High=10**) | Escala da aplicação (convenção D7/T-850-01), NÃO ordinais IDL |
| `ModelSpecialization` (Text=0, Vision=1, Embedding=2, Transcription=3) | `can_serve()`: Text é fallback genérico; Vision aceita Text+Vision; demais só elas mesmas |
| `AgentHealth` (Offline=0, Degraded=1, Healthy=2) | Saúde do agente |
| `FinishReason` (None=0..Error=4) | Vocabulário string canônico `as_str()`/`parse()` (o IDL usa string em `Task` e i32 em `TaskOutput`) |
| `ComponentType` (Orchestrator=0, Agent=1, LlamaServer=2, Client=3) | Origem de métricas |
| `SecurityLevel` (Public=0, Internal=1) | Só 2 níveis com evidência (ver Limites) |
| `ToolCallStatus` (Pending=0..Failed=5) | `Display` maiúsculo p/ logs; terminais 2\|4\|5 |
| `FuzzyMetrics` (8×f64) | urgency, deadline_pressure, recent_latency, agent_load, error_rate, historical_confidence, estimated_complexity, streaming_need; `to_array()` em ordem canônica |
| `instrumentation::{LatencySpan, RttTracker, ErrorCounter}` | Span T1–T6 (ns), RTT médio + EMA (α=0.1), erros total + 8 categorias (atômicos) |

Padrão: conversões wire→enum são totais via `TryFrom<i32>` com `UnknownEnumValue`
(nome do enum + valor), nunca `panic!`.

## Como compilar e testar

```bash
cd src/rust
cargo check -p orch-common
cargo test -p orch-common        # 9 testes unitários inline (discriminantes, terminais, can_serve, EMA, contadores, span)
cargo clippy -p orch-common -- -D warnings
```

Sem binários, features ou env vars — é `rlib` puro (`serde`, `serde_json`,
`thiserror`, `tracing`, `parking_lot`, `ahash`, todas do workspace).

## Limites

- `SecurityLevel` modela só Public/Internal; o "etc." do IDL não tem valor confirmado.
- `FinishReason::parse` só reconhece o vocabulário: strings operacionais fora dele
  (ex. `"MAX_RETRIES_EXCEEDED"` no wire de `Task`, que é string livre) retornam `None`.
- `ownership_strength = 0` nos perfis QoS é placeholder — a ordenação real de claim
  vem de `dds-contract::roles` (CLIENT=10 < AGENT=100 < ORCHESTRATOR=200).
