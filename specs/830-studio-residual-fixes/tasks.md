# specs/830-studio-residual-fixes — Lacunas residuais do Studio (fase 800)

Fonte: auditoria "Studio 100% implementado?" (2026-10-04) — `THESIS_RUST_WORKSPACE_REPORT.md` §6 e findings de audit. Escopo: apenas correções locais e testáveis; **multi-host/SSH/mDNS/studio-storage/auth permanecem bloqueados por ambiente** (26 gates 🔒 da fase 800) e fora deste incremento.

| Task | Escopo | Estado |
|---|---|---|
| T-830-01 | `orchestrator-studio/src/agents.rs`: chamada HTTP bloqueante na thread de UI → padrão worker+mpsc+poll (T-820-19); teste wire | [x] |
| T-830-02 | `catalog_remote.rs`/`views/shared_catalog.rs`: integrar `events_since` (cursor, poll, re-snapshot em 410); teste de wire | [x] |
| T-830-03 | `studio-core`: `Catalog::publish/delete` usam `RevisionGuard`/`GenerationGuard` (REQ-800/801 integrados; `generation` deixa de ser ignorado); testes de geração | [x] |
| T-830-04 | Painel "Catálogo" local repontado para snapshot do nó (leitura) ou removido — o que quebrar menos testes | [x] |
| T-830-05 | `studio-node`: default de `STUDIO_NODE_DB` em `$HOME/.local/share/studio-node/operations.json` (persistente por padrão); `lib.rs` doc corrigida; `env.example` sem `%h` | [x] |
| T-830-06 | `models.rs`: `STUDIO_MODELS_DIR` → fallback `$HOME/tese/models`; sem path hardcode de usuário | [x] |
| T-830-07 | packaging: `.desktop` com `Exec=studio` (PATH); coerência unit/env | [x] |
| T-830-08 | fase 800: `REPORT.md` criado; `coverage.md` inclui T-800-21..23; `spec.md` REQ-803/804 atualizados | [x] |
| T-830-09 | Integração: gates por crate + workspace; commits `[T-830-XX]`; notes.md | [x] |
