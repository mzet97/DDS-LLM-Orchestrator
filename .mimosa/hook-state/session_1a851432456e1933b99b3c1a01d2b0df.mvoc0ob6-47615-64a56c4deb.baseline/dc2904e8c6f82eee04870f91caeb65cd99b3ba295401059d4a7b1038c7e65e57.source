//! # studio-core
//!
//! Domínio administrativo puro do DDS Orchestrator Studio (fase 800): revisões
//! condicionais, gerações de intenção e, em breve, snapshot/cursor e tombstones.
//! Sem IO e sem rede; depende só de `thiserror` (erros) e `serde` (tipos de
//! fio do §34: snapshot, eventos, revisões) — as decisões de concorrência
//! vivem aqui para que GUI, nó e autoridade compartilhem a mesma semântica
//! testada (prompt SDD §§12–13, 34; RF-31/32).

pub mod catalog;
pub mod revision;

pub use catalog::Catalog;
