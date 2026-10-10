# Snapshots ABI do `cyclonedds-rust-sys` para cross-compile

O build.rs do `cyclonedds-rust-sys` (FFI-ABI-008) exige um snapshot de layout
(`offsetof`/`sizeof` do probe C) por TARGET quando host ≠ target: sem ele, o
cross-compile para musl falha.

## `x86_64-unknown-linux-musl.rs` (2026-10-06)

Gerado compilando o `probe.c` do próprio crate com o toolchain musl
(musl.cc 11.2.1, `-static`) e executando no host glibc. **Verificação: output
byte-idêntico ao probe nativo glibc** (`diff` vazio) — os layouts do CycloneDDS
não dependem da libc no x86_64 LP64.

## Como reinstalar (se o registry for re-extraído)

```bash
cp abi-snapshots/x86_64-unknown-linux-musl.rs \
  ~/.cargo/registry/src/index.crates.io-*/cyclonedds-rust-sys-1.2.1/abi/
```

O `scripts/deploy-studio-node.sh` verifica e instala automaticamente.
O caminho definitivo é incluir o snapshot no prerelease da crate (T-814).
