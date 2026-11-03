//! Autoridade de catálogo compartilhado no nó (P2a; G-44/45/47/49/57).
//!
//! Reutiliza `studio_core::Catalog` (mesma semântica condicional da GUI).
//! Persistência por journal JSONL event-sourced: cada mutação aceita é
//! anexada com `sync_all`; o boot reconstrói por replay e falha rápido em
//! linha corrompida (operador corrige o arquivo, nunca máscara).
//!
//! Durabilidade (REQ/T-820-18): o journal é write-ahead do ponto de vista
//! do chamador — a mutação só é reportada como aceita se o append persistiu.
//! Em falha de append a memória é reconstruída por replay (rollback ao
//! estado do disco), então memória e journal nunca divergem.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use studio_core::catalog::{Catalog, CatalogError, Cursor, DefinitionId, Event, Snapshot};
use studio_core::revision::{Generation, Revision};
use thiserror::Error;

/// Entrada do journal: a mutação aceita, na ordem aplicada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JournalEntry {
    Publish {
        id: String,
        base: Option<u64>,
        value: String,
        generation: u64,
    },
    Delete {
        id: String,
        base: u64,
    },
}

/// Erros da autoridade (fronteira HTTP ↔ catálogo + disco).
#[derive(Debug, Error)]
pub enum AuthorityError {
    /// Conflito de revisão ou id ausente onde exigido (G-47).
    #[error("conflito de revisao")]
    Conflict { current: Option<u64> },
    /// Geração de intenção regressiva/repetida sobre item vigente (REQ-801).
    #[error("geracao obsoleta: proposta {got}, vigente {current}")]
    StaleGeneration { got: u64, current: u64 },
    /// Id com tombstone vigente (G-57).
    #[error("id removido")]
    Tombstoned { deleted_at: u64 },
    /// Cursor fora da retenção: ressincronizar (G-49).
    #[error("cursor expirado")]
    CursorExpired,
    /// Journal ilegível ou corrompido.
    #[error("journal corrompido em {path}: {detail}")]
    Corrupt { path: String, detail: String },
}

impl AuthorityError {
    /// Mapeamento exaustivo do erro do domínio.
    #[must_use]
    pub fn from_catalog(err: CatalogError) -> Self {
        match err {
            CatalogError::Conflict { current } => Self::Conflict {
                current: current.map(|revision| revision.0),
            },
            CatalogError::StaleGeneration { got, current } => Self::StaleGeneration {
                got: got.0,
                current: current.0,
            },
            CatalogError::Tombstoned { deleted_at } => Self::Tombstoned {
                deleted_at: deleted_at.0,
            },
            CatalogError::CursorExpired => Self::CursorExpired,
        }
    }
}

/// Catálogo autoritativo do nó com journal opcional.
#[derive(Debug)]
pub struct CatalogAuthority {
    catalog: Catalog,
    journal: Option<PathBuf>,
}

impl Default for CatalogAuthority {
    fn default() -> Self {
        Self::new()
    }
}

mod catalog_auth_impl;

#[cfg(test)]
mod tests;
