# `studio-ssh` — bridge SSH do Studio (russh puro, G-04)

> Crate: `src/rust/crates/studio-ssh/` · Só lib `studio_ssh` (sem binário) ·
> Fase 800 (T-800-28), confiança G-02.

## Propósito

Bridge SSH administrativa do Studio, 100% `russh` integrada ao binário: sem
`ssh`/`scp`/`sshpass`/shell, sem senha, sem agente, sem encaminhamento.
Uma identidade Ed25519 dedicada por instalação (privada cifrada 0600, senha
nunca gravada), confiança verificada contra a chave **efetivamente
apresentada no handshake**: host desconhecido exige aprovação explícita do
operador, chave alterada bloqueia sem re-tentativa. Só autenticação
`publickey`; falha de auth é erro, nunca tentativa de outro método.

## API pública

- **`identity`** (`identity::{...}`): `generate(dir, passphrase)` (Ed25519
  aleatório; `studio_ed25519` cifrado 0600 + `.pub` em linha
  `authorized_keys`; senha vazia recusada), `unlock(private_pem, passphrase)`
  (desbloqueio explícito a cada carga), `public_fingerprint` (`SHA256:…`).
  Erros `IdentityError::{Storage, Unlock}`.
- **`trust`** (`trust::{...}`): `TrustStore::{new, from_approvals, approvals,
  check, approve}` — `check` retorna `Unknown` (novo) ou `Changed`
  (rotação; aprovação posterior registra `replaced`); `TrustFile::{open,
  store, store_mut, save}` — JSON de aprovações (`project/alias/host/
  port/key/approved_by/approved_at_unix/replaced`), 0600 em Unix, ausente
  cria vazio, corrompido é erro. Erros `TrustError::{Unknown, Changed,
  Unreadable}`.
- **`bridge`** (`bridge::{...}`): `run_command(target, private_key, trust,
  command)` (async: conecta, verifica o handshake, autentica `publickey`,
  executa e devolve o stdout UTF-8 quando o status é zero; status ≠ 0 vira
  `CommandFailed{status, stdout, stderr}` e morte por sinal vira
  `CommandSignaled{signal, stdout, stderr}`),
  `run_command_blocking(target, private_pem, passphrase, trust_path,
  command)` (caminho síncrono p/ GUI sem runtime — destinado a thread
  dedicada, nunca à thread de UI). `SshTarget{host, port, username}`.
  Erros `BridgeError::{UnknownHost, ChangedKey, AuthFailed, Transport,
  CommandFailed, CommandSignaled, Identity, Trust}`.
- Raiz reexporta `bridge` + `identity` + `trust` e `russh::keys::ssh_key`.

## Testes

```bash
cd src/rust
cargo test -p studio-ssh   # 16 testes: 7 units + 7 bridge + 2 identidade
```

Integração `bridge_isolated` (servidor russh descartável em loopback, host
key por teste): desconhecido bloqueia com a impressão real, aprovação
persistida sobrevive a reabertura, rotação bloqueia sem re-aceite, chave
errada falha sem tentativa de senha (contador 0), status ≠ 0 vira
`CommandFailed` com stdout/stderr, morte por sinal vira `CommandSignaled`,
arquivos nunca contêm a senha. `identidade_aleatoria`: mesma senha gera
públicas diferentes;
desbloqueio preserva a fingerprint. Units: identidade (senha vazia, ciclo
gera/desbloqueia, linha `.pub`) e cofre (desconhecido, host≠porta,
rotação+`replaced`, roundtrip em arquivo).

## Limites

- **Standalone, sem consumidor na GUI**: a integração (`ssh_session.rs`,
  `ssh_trust.rs`, `views/ssh.rs`, painel "SSH dedicado") foi revertida no
  merge `5004a05` (protótipo de setembro superado pelo UX4; vivo no
  histórico). Religar na GUI é a **T-800-29 (aberta)**; prova contra host
  real segue para o canário (exige cadastrar a `.pub`).
- Sem teste de `run_command_blocking` (só o caminho async é exercitado).
- Certificados OpenSSH nunca são autoridade: registrados como apresentados
  e rejeitados por desenho.
- `Verifier` usa `.lock().expect(...)` (mutex interno, sem contenção real).
