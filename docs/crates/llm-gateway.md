# `llm-gateway` — roteamento de inferência com failover

> Crate: `src/rust/crates/llm-gateway/` · Só biblioteca, sem binário ·
> Substitui `src/orchestrator/llm_gateway/` (~1k LOC Python).

## Propósito

Roteamento de inferência LLM a provedores (local llama-server C++ vs cloud):
multi-worker real via `Semaphore`, cache de resultados (FNV-1a + FIFO),
rate-limit token-bucket com 429 retriável, e failover em cascata com circuit
breaker por target (T-420/421/422/424). O `orchestrator` monta as cadeias de
failover; o consumo é via `LlmGateway::with_providers` + `process_routed`.

## Componentes

- **`LlmGateway`**: semáforo de workers + limiter + cache + métricas.
  `process()` **retorna erro de propósito**, forçando `process_routed()`.
  `process_routed()`: cache antes do rate-limit → roteamento → failover com
  breaker (`request_id-fo`).
- **`GatewayProviders`**: local/cloud + `register_failover`; `route` — `Any`
  prefere local. `Provider::{Local,Cloud}`, `ProviderConstraint::{Any,LocalOnly,
  CloudOnly}` (+ literais IDL).
- **`LlmCache`**: DashMap+ahash, eviction FIFO real por contador monotônico.
  `cache_key()`: FNV-1a de (constraint, model, messages) com prefixos de
  comprimento; `agent_id`/temp/tokens fora da chave (cross-agent).
- **`RateLimiter`**: token-bucket (`try_acquire`).
- **`GatewayError`** → `to_llm_error()` mapeia para `LLMInferenceError` no fio
  (429/503/500/504/400). **`GatewayMetrics`**: atômicos total/cache_hits/
  rate_limited/errors/failover_*.
- **Failover** (`failover.rs`): `CircuitBreaker` 3 estados (closed/open/half-open
  com sonda única; default 3 falhas/30 s), `FailoverStrategy::{Priority,
  RoundRobin}`, `FailoverManager` (breakers por chave), `FailoverConfig/
  TargetConfig/Result/Event`, `HealthStatus`.

## API pública

`LlmGateway::{new, with_providers, process, process_routed, metrics}`,
`GatewayProviders::{new, register_failover, get_failover_targets,
any_failover_available, route, provider_name_from_constraint}`, trait
`LlmProvider` (`infer`, `kind`), `MockProvider` (testes), `Provider`,
`ProviderConstraint`, `GatewayError`, `GatewayMetrics`, `RateLimiter`,
`LlmCache`, `FailoverTarget` + reexports de failover.

## Testes

```bash
cd src/rust
cargo test -p llm-gateway   # 12 testes (tests/gateway.rs) + unitários, sem DDS
```

Sem features, env vars ou flags — configuração é programática
(`GatewayProviders`, breakers, limiter, cache).

## Limites

- **Sem runtime DDS próprio**: não há bin, loop de assinatura
  `LLM.InferenceRequest`, nem providers local/cloud reais (só `MockProvider`).
  A fiação DDS↔HTTP/streaming externo vive fora deste crate.
