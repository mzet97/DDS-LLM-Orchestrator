# `mcp-gateway` — fronteira MCP via `ToolCall.Request`

> Crate: `src/rust/crates/mcp-gateway/` · Port de `src/orchestrator/mcp_gateway/`
> (~836 LOC Python) · Binário `mcp-gateway` (exige `dds`).

## Propósito

Expõe ferramentas ao barramento DDS via tópico `ToolCall.Request` (chave `call_id`),
com governança fail-closed por snapshot/update DDS versionado ANTES de executar.
Ciclo: filtra `PENDING` → política nega? `DENIED`+mensagem byte-idêntica ao Python :
grava `EXECUTING` na mesma instância → dispatch → `COMPLETED`+`{"result":…}` ou
`FAILED`+`error_message`, com `completed_at_ns`. Filesystem real (3 ops);
github/web/database/cicd = 14 stubs `NotConfigured`. Exactly-once via claims
duráveis (REQ-706); sobrecarga responde FAILED sem claimar (REQ/T-820-17).

## Como rodar

```bash
cd src/rust
CYCLONEDDS_STATIC=1 cargo run -p mcp-gateway --features dds -- \
  --dds-domain 0 --filesystem-root /tmp/sandbox [--dds-secure --dds-security-dir DIR]
# --sandbox-dir = alias histórico. Sem --dds-secure, avisa "local-only". Logs JSON. Ctrl+C encerra.
```

Features: `dds`, `security` (= DDS Security). `--dds-security-dir` exige 6 arquivos
(identity/participant cert+key, governance, permissions ×2). `RUST_LOG` (default
`info`). `GITHUB_TOKEN`/`DATABASE_URL` só aparecem nas mensagens `NotConfigured`
(não são lidos — stubs).

## Componentes

- **`ToolCallService<D: DataSpaceApi>`** (`service.rs`): política+registry+claims.
  `process_one` (DENIED→EXECUTING→dispatch→terminal, mesma instância). `run`
  (JoinSet, `MAX_INFLIGHT_JOBS=64`, drena sempre; overload→`reject_overloaded`:
  FAILED+`OVERLOAD_MESSAGE` só se ainda PENDING, best-effort).
- **`ToolRegistry`** (DashMap; solta o shard antes do `await`): `register`,
  `dispatch`, `list_tools`. Trait `ToolHandler` object-safe (sem async-trait).
- **`DistributedPolicy`** (`policy.rs`): fail-closed (nega sem snapshot; max-age
  300 s; skew futuro 5 s, T-890-06). `ingest_snapshot[_at]` valida envelope+versão
  e rejeita regressão/timestamp. `evaluate[_at]`: nível→requester→snapshot→
  expiração→tool_call→llm_request; só `Allowed` estrito passa (sem snapshot, tudo nega).
- **Claims** (`claim.rs`, REQ-706): trait `ClaimStore::try_claim`; `MemoryClaimStore`
  (testes); `FileClaimStore` (O_EXCL + openat2 BENEATH/NO_SYMLINKS, nome=hex(call_id),
  fsync; **sem lease** — decisão documentada).
- **Filesystem** (`tools/filesystem.rs`): 3 ops (read/write/list) com `FsLimits`
  (1 MiB/1 MiB/1000); msg de write idêntica ao MCP oficial. **`SandboxRoot`**
  (`tools/sandbox.rs`): fd fixo, openat2 BENEATH, bloqueia `.mcp-claims`, `..`,
  absolutos e symlinks; `list` filtra `.`/`..`/`.mcp-claims`.
- **Externas** (`tools/external.rs`): 14 stubs github×4/web×2/database×4/cicd×4.
- **`dds.rs`**: `build_service` (DataSpace strength 200 p/ ownership Exclusive +
  registry + claims em `<raiz>/.mcp-claims`). Reutiliza o tópico canônico do
  DataSpace (não criar 2º Topic).

Status (`service.rs::status`): PENDING..FAILED + `DENIED_MESSAGE` (byte-idêntica).
`ToolError`: 7 variantes + `code()` + `to_error_json()`.

## Testes

```bash
cd src/rust
cargo test -p mcp-gateway
CYCLONEDDS_STATIC=1 cargo test -p mcp-gateway --features dds -- --test-threads=1
CYCLONEDDS_STATIC=1 cargo test -p mcp-gateway --features security -- --test-threads=1
```

Sem DDS: policy (244 L), filesystem (288 L), registry, exactly_once, overload,
sandbox_adversarial. Com DDS: claim, dds_policy, dds_policy_recovery,
exactly_once_dds + helpers `tests/common/` (TempDir).

## Limites

- 14 ferramentas externas = stubs `NotConfigured` (sem HTTP/Postgres/HTML).
- Claims sem lease/heartbeat: vencedor que crasha entre claim e terminal deixa a
  call em EXECUTING para sempre (recuperação: reemitir com novo `call_id`).
- `tool_call` exige `Allowed` estrito também em `check_llm_request`.
- `list_directory` omite `.mcp-claims` (claims vivem sob a raiz do sandbox).
- Único dos serviços que depende de crate irmã (`policy-engine`).
