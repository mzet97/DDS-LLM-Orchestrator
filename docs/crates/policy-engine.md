# `policy-engine` — fonte da verdade das políticas

> Crate: `src/rust/crates/policy-engine/` · Port de `src/orchestrator/policy_engine/` ·
> Binário `policy-engine` (compila sem `dds`, mas sem `dds` só avisa e sai).

## Propósito

(a) Avaliação local de `ToolCallRequest` (fast path: nível máximo,
whitelist/blacklist, rate limit 60 s); (b) avaliação das regras de `policies.json`
(`llm_inference` e `tool_call`, com ALLOW_ALL quando documento vazio); (c) cache
local com TTL 300 s (DashMap — substitui Redis/`_NullRedis`); (d) serviço fonte
da verdade: lê `policies.json`, publica `Security.PolicySnapshot` (re-publica a
cada 60 s p/ late-joiners) e aplica deltas de `Security.PolicyUpdate`
(`ADD/UPDATE/REMOVE_RULE`).

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p policy-engine --features dds -- \
  --dds-domain 0 --policy-file crates/policy-engine/policies.json
# Flags: --dds-domain, --policy-file (default <CARGO_MANIFEST_DIR>/policies.json),
# --republish-interval-secs (60), --log-level. Ctrl+C encerra.
```

## Componentes

- **`LocalPolicyEngine`** (`engine.rs`): defaults Confidential, listas vazias,
  60/min. `evaluate`: nível→whitelist→blacklist→rate limit→Allowed.
  `check_rate_limit_at` com clock injetado (1ª chamada prima sem contar; prune
  anti-leak). `SecurityLevel` 0..3 (`security_level_name`; inválido→"INVALID").
- **`PolicyDocument`** (`rules.rs`): Value JSON + `version`. `check_llm_request`
  (allowed_agents + sufixo `<agente>-` + níveis por role). `check_tool_call`
  (allowlist por agente + `high_risk_tools` sempre DENY). `apply_delta` (deep
  merge p/ ADD/UPDATE, remoção de folhas p/ REMOVE).
- **`PolicyCache`** (`cache.rs`): get/set/delete, TTL default 300 s, clock injetado.
- **`PolicyEngineService<D: DataSpaceApi>`** (`service.rs`): genérico (DDS real ou
  InMemory). `load_and_publish_inner` com dedupe versão+conteúdo e guarda
  anti-regressão (REQ/T-820-16). `handle_update` rejeita `new_version<=atual`
  (`StaleVersion`); divergência de previous só avisa (last-writer-wins). `run`:
  carga inicial + `subscribe_security_updates` + ticker 60 s (erro de republish
  não derruba).
- **`policies.json`**: documento real v2 (4 agentes, `model_routing`,
  `security_routing`, `tool_call` com `filesystem.write_file` high-risk).

Decisões: `PolicyDecision::{Allowed, AllowedNoPolicy, Denied}`. Consts:
`DEFAULT_REPUBLISH_INTERVAL=60s`, `DEFAULT_POLICY_ID="default"`,
`DEFAULT_PUBLISHED_BY="policy-engine-v1"`.

## Testes

```bash
cd src/rust
cargo test -p policy-engine
CYCLONEDDS_STATIC=1 cargo test -p policy-engine --features dds -- --test-threads=1
```

Suíte: `engine` (nível/listas/rate), `rules` (usa o `policies.json` real),
`cache` (TTL), `service_inmemory` (snapshot+delta+cache, 266 L), `dds_roundtrip`
(dom 92, TRANSIENT_LOCAL late-join + update→republicação).

## Limites

- Identidade do agente = `requester_id` (o IDL não tem `agent_id`).
- Rate limit Rust é funcional mas diverge do Python (bug latente no Python nunca
  negava) — semântica própria documentada.
- Ramo `provider_constraint=="LOCAL_ONLY"` fica no gateway, não na política.
- Deltas exigem JSON objeto na raiz; `previous_version` divergente aplica mesmo
  assim (só warning).
- `apply_delta`/`handle_update` são extensão Rust sem contraparte Python.
