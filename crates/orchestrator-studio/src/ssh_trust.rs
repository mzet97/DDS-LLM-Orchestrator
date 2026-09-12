//! Confiança de hosts na GUI: reexporta o cofre de [`studio_ssh`].
//!
//! A lógica pura vive em `studio_ssh::trust` (mesmos tipos e regras do
//! antigo módulo local, agora com persistência e handshake real); este
//! módulo só mantém o caminho de import da GUI estável.

pub use studio_ssh::trust::{Approval, KeyIdentity, TrustError, TrustFile, TrustStore};
