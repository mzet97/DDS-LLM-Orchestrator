# Release checklist — `cyclonedds-rust-sys` 1.2.2 (prerelease T-814)

**Preparado:** 2026-10-06 · **Base publicada:** 1.2.1 (crates.io) · **Publish:** bloqueado por token crates.io do autor (passo a passo em `REPORT.md` §5)

## Changelog (copiar para o `CHANGELOG.md` do fork como `[1.2.2] - <data>`)

### Added

- **Snapshot ABI `x86_64-unknown-linux-musl`** (`abi/x86_64-unknown-linux-musl.rs`):
  habilita cross-compile para musl sem o pânico `FFI-ABI-008`. Gerado pelo
  próprio `probe.c` do crate compilado com musl-gcc 11.2.1 (musl.cc, `-static`)
  e **verificado byte-idêntico ao output do probe nativo glibc** — os layouts
  sondados (offsets de `ddsi_serdata`/`ddsi_serdata_ops`, `sizeof` dos status
  structs) dependem do ABI x86_64 LP64, não da libc.
- **Receita de cross-compile documentada** (usada e validada na tese, fase 890):
  toolchain musl no PATH, `CC`/`AR` apontando o musl-gcc **na mesma invocação**
  (o CMake só honra `CC` na primeira configuração — builds stale deixam objetos
  glibc dentro do rlib), `CYCLONEDDS_STATIC=1`, e
  `cargo clean -p cyclonedds-rust-sys` ao trocar de toolchain.
- **Consumo comprovado:** binário musl estático (`static-pie`) do runtime da
  tese com CycloneDDS C vendored compilado por este build.rs, deployado em 3
  hosts Ubuntu 24.04/glibc 2.39 com descoberta DDS ativa.

### Fixed

- (nenhum — sem mudança de código do crate nesta release; o diff 1.2.1→1.2.2 é
  só o snapshot + docs. Se outros fixes entrarem antes do publish, listá-los
  aqui com o gate de regressão correspondente.)

## Passos de publish (executar no fork, branch de release)

1. `CHANGELOG.md`: `[Unreleased]` → `[1.2.2] - <data>` com o texto acima.
2. `cyclonedds-rust-sys/Cargo.toml`: `version = "1.2.2"`.
3. Conferir emparelhados (`cyclonedds`, `cyclonedds-build`) — bump só se houver
   mudança neles; caso contrário manter.
4. Gates do fork: `cargo test --workspace` e `cargo clippy --workspace
   --all-targets -- -D warnings`.
5. `cargo package -p cyclonedds-rust-sys --list` — confirmar que `abi/*.rs`
   entra no pacote (`.cargo-ok` não; `Cargo.lock` do pacote regenera).
6. `cargo publish -p cyclonedds-rust-sys` **(exige token crates.io do autor)**.
7. No runtime da tese (`src/rust/Cargo.toml:38`): `=1.2.1` → `=1.2.2`;
   `cargo update -p cyclonedds-rust-sys`; repetir gates do consumidor
   (fmt/clippy/`cargo test --workspace`, 449/0 atual) + smoke musl
   (`scripts/deploy-studio-node.sh agent@<host> dds-agent`) antes de deploy.
8. Registrar aqui: data do publish, versão, SHA do fork e do runtime.

## Contexto

- O gate FFI-ABI-008 (exigência de snapshot por target em cross-compile) já
  existe na 1.2.1 publicada; a 1.2.2 entrega o snapshot que faltava para musl.
- Sem o publish, o runtime segue funcional com `=1.2.1` + snapshot instalado no
  registry local (`src/rust/abi-snapshots/` — reinstalação automática pelo
  deploy script).
