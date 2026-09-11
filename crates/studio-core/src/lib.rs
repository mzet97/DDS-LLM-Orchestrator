//! # studio-core
//!
//! Domínio administrativo puro do DDS Orchestrator Studio (fase 800): revisões
//! condicionais, gerações de intenção e, em breve, snapshot/cursor e tombstones.
//! Sem IO, sem rede e sem dependências além de `thiserror` — as decisões de
//! concorrência vivem aqui para que GUI, nó e autoridade compartilhem a mesma
//! semântica testada (prompt SDD §§12–13, 34; RF-31/32).

pub mod catalog;
pub mod revision;
