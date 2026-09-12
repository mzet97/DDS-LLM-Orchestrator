//! # studio-node
//!
//! Esqueleto do nó local do DDS Orchestrator Studio (fase 800, P2, T-800-05).
//! Protocolo administrativo versionado ([`protocol`]) mais log idempotente de
//! operações com reconciliação ([`operations`]) — G-05/06 em integração local.
//! Sem transporte e sem persistência em disco nesta fase: o log vive em
//! memória e o fio é JSON via serde (transporte e `studio-storage` vêm depois).

pub mod operations;
pub mod probe;
pub mod protocol;
pub mod server;
