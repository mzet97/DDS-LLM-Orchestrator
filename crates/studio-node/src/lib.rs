//! # studio-node
//!
//! Nó local do DDS Orchestrator Studio (fase 800, P2). Protocolo
//! administrativo versionado ([`protocol`]) servido em HTTP localhost
//! ([`server`], T-800-06), log idempotente de operações com reconciliação
//! ([`operations`]) — G-05/06 em integração local — e catálogo compartilhado
//! autoritativo com journal JSONL ([`catalog_auth`], REQ/T-800/801,
//! REQ/T-820-18).
//!
//! Persistência (T-830-05): `STUDIO_NODE_DB` aponta o JSON do log de
//! operações (o journal do catálogo é derivado dele); sem a variável, o
//! padrão é `$HOME/.local/share/studio-node/operations.json`, criado on
//! demand — `HOME` ausente degrada para memória volátil com aviso no boot.

pub mod actuator;
pub mod catalog_auth;
pub mod operations;
pub mod probe;
pub mod protocol;
mod routes_catalog;
mod routes_services;
pub mod server;
