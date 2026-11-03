//! Bridge SSH do Studio: chave dedicada + confiança verificada no handshake.
//!
//! Sem `ssh`/`scp`/`sshpass`/shell: transporte 100% `russh` integrado ao
//! binário. Sem senha, sem agente, sem encaminhamento.

pub mod bridge;
pub mod identity;
pub mod trust;

pub use bridge::{run_command, run_command_blocking, BridgeError, SshTarget};
pub use identity::{generate, public_fingerprint, unlock, IdentityError, IdentityPaths};
pub use russh::keys::ssh_key;
pub use trust::{Approval, KeyIdentity, TrustError, TrustFile, TrustStore};
