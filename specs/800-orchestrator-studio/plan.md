# Fase 800 — plano

## Como (vertical, test-first, sem mocks de integração)

1. `studio-core`: tipos administrativos puros (revisão, geração, snapshot/cursor,
   tombstone) sem IO, sem rede, sem dependência nova — só `thiserror`.
2. Cada comportamento nasce em teste de aceite antes/depois do código mínimo,
   no mesmo incremento (loop SDD do `AGENTS.md`).
3. Integrações reais (SSH/systemd/mDNS/SQLite/egui/DDS) entram em crates próprias
   (`studio-node`, `studio-integration`, `studio-storage`, `orchestrator-studio`)
   somente com plano pequeno por trilha (§29) e gates G-01..G-70 correspondentes.
4. Reutilizar, não duplicar: `agent`/`client/wf-run`/`det-responder`/`llm-gateway`/
   `mcp-gateway`/`policy-engine`/`context-store`/`observability` e tipos de
   `dds-contract` (ver `audit.md`). Nada de `wf/assembly.rs` duplicado.

## Arquivos deste incremento

- `specs/800-orchestrator-studio/{spec,plan,tasks,audit}.md`
- `crates/studio-core/{Cargo.toml,src/lib.rs,src/revision.rs}` (+ testes inline)
- `Cargo.toml` (workspace): registra `crates/studio-core`.

## Comandos de verificação (por task)

```bash
cd src/rust
cargo test -p studio-core
cargo clippy -p studio-core --all-targets -- -D warnings
cargo fmt --all --check
```

Sem commit/push sem autorização (§29 do prompt SDD): o estado `[x]` abaixo significa
teste verde + lints verdes, com commit pendente de autorização explícita.
