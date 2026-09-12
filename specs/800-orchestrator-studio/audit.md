# Fase 800 — auditoria P0 (REQ-802, verificado nesta execução)

Mapa reutilizar × não-duplicar do runtime existente. Tudo abaixo foi lido no
checkout nesta sessão; rótulos §2.1 do prompt.

## Reutilizar (o Studio referencia, não reimplementa)

- `agent` (bin+lib): flags `--agent-id/--dds-domain/--slots/--model/
  --specialization/--engine/--llama-url/--provider-constraint`
  (`crates/agent/src/main.rs:27-58`); engines mock|http|dds. O Studio implanta
  este binário com argv gerado — não simula agentes.
- `client` + `wf-run`: workloads `seq_chain_v1`, `fork_join_v1`,
  `fork_join_serial_v1` (`crates/client/src/bin/wf_run.rs:178-180,251-255`,
  bin `wf-run` em `crates/client/Cargo.toml:39-40`). Executar via runner real.
- `det-responder`: backend DDS determinístico (`LLM.*`, `--model`,
  `crates/det-responder/src/main.rs:52-87`). Diagnóstico/teste, não LLM real.
- `llm-gateway`: **somente lib** (sem `[[bin]]` no `Cargo.toml`); `Provider`,
  `ProviderConstraint`, `GatewayError` (`crates/llm-gateway/src/lib.rs:29-102`).
  Não oferecer botão que inicie executável inexistente.
- `mcp-gateway`: `ToolCall.Request` atualizado na mesma instância, chave
  `call_id` (`crates/mcp-gateway/src/service.rs:6,107-130`). Não inventar
  `ToolCall.Response`.
- `policy-engine`, `context-store` (journal JSONL), `observability`: consumir
  APIs/dados existentes; não migrar persistência por causa do Studio.
- Contratos: `src/llama_cpp/dds/idl/OrchestratorDDS.idl` e
  `src/llama_cpp/dds/v4/idl/OrchestratorV4.idl`; tipos gerados via idlc em
  `crates/dds-contract` (Constituição Art. I).

## Divergências registradas (não reconciliadas em silêncio)

- Prompt §2.4 cita `third_party/llama.cpp_dds/...` e `src/llama_cpp/dds/` como
  espelho: a entrada de geração válida observada é `src/llama_cpp/dds/idl/` +
  `v4/idl/` acima; lido, não validado contra o C++ gerado (fora deste incremento).
- `cargo fmt --all --check` falha em `crates/spike-interop/build.rs`
  (pré-existente, fora do escopo); `cargo fmt -p studio-core -- --check` verde.

## Coberto neste incremento

- T-800-01: `RevisionGuard`/`GenerationGuard` em `crates/studio-core`
  (3 testes verdes, clippy `-D warnings` limpo, fmt limpo).
