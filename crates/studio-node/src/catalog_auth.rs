//! Autoridade de catálogo compartilhado no nó (P2a; G-44/45/47/49/57).
//!
//! Reutiliza `studio_core::Catalog` (mesma semântica condicional da GUI).
//! Persistência por journal JSONL event-sourced: cada mutação aceita é
//! anexada com `sync_all`; o boot reconstrói por replay e falha rápido em
//! linha corrompida (operador corrige o arquivo, nunca máscara).

use std::io::Write;
use std::path::{Path, PathBuf};

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

impl CatalogAuthority {
    /// Autoridade em memória (testes e nós sem persistência).
    #[must_use]
    pub fn new() -> Self {
        Self {
            catalog: Catalog::new(),
            journal: None,
        }
    }

    /// Autoridade com journal: reconstrói por replay; falha rápido se
    /// corrompido. Arquivo ausente = primeiro boot.
    pub fn with_journal(path: PathBuf) -> Result<Self, AuthorityError> {
        let path_str = path.display().to_string();
        let corrupt = |detail: String| AuthorityError::Corrupt {
            path: path_str.clone(),
            detail,
        };
        let mut authority = Self {
            catalog: Catalog::new(),
            journal: Some(path),
        };
        let journal = authority.journal_path().to_path_buf();
        let text = match std::fs::read_to_string(&journal) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(authority),
            Err(err) => return Err(corrupt(err.to_string())),
        };
        for (line_no, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(line)
                .map_err(|err| corrupt(format!("linha {line_no}: {err}")))?;
            authority
                .replay(entry)
                .map_err(|detail| corrupt(format!("linha {line_no}: {detail}")))?;
        }
        Ok(authority)
    }

    fn journal_path(&self) -> &Path {
        self.journal.as_deref().expect("com journal")
    }

    fn replay(&mut self, entry: JournalEntry) -> Result<(), String> {
        match entry {
            JournalEntry::Publish {
                id,
                base,
                value,
                generation,
            } => self
                .catalog
                .publish(
                    DefinitionId(id),
                    base.map(Revision),
                    value,
                    Generation(generation),
                )
                .map(|_| ())
                .map_err(|err| err.to_string()),
            JournalEntry::Delete { id, base } => self
                .catalog
                .delete(&DefinitionId(id), Revision(base))
                .map(|_| ())
                .map_err(|err| err.to_string()),
        }
    }

    fn append(&self, entry: &JournalEntry) -> Result<(), AuthorityError> {
        let Some(path) = self.journal.as_deref() else {
            return Ok(());
        };
        let mut line = serde_json::to_string(entry).map_err(|err| AuthorityError::Corrupt {
            path: path.display().to_string(),
            detail: err.to_string(),
        })?;
        line.push('\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| AuthorityError::Corrupt {
                path: path.display().to_string(),
                detail: err.to_string(),
            })?;
        file.write_all(line.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|err| AuthorityError::Corrupt {
                path: path.display().to_string(),
                detail: err.to_string(),
            })
    }

    /// Publica condicionalmente e journaliza o aceito (G-47/57).
    pub fn publish(
        &mut self,
        id: String,
        base: Option<u64>,
        value: String,
        generation: u64,
    ) -> Result<u64, AuthorityError> {
        let revision = self
            .catalog
            .publish(
                DefinitionId(id.clone()),
                base.map(Revision),
                value.clone(),
                Generation(generation),
            )
            .map_err(AuthorityError::from_catalog)?;
        self.append(&JournalEntry::Publish {
            id,
            base,
            value,
            generation,
        })?;
        Ok(revision.0)
    }

    /// Exclui com tombstone e journaliza (G-57).
    pub fn delete(&mut self, id: String, base: u64) -> Result<u64, AuthorityError> {
        let at = self
            .catalog
            .delete(&DefinitionId(id.clone()), Revision(base))
            .map_err(AuthorityError::from_catalog)?;
        self.append(&JournalEntry::Delete { id, base })?;
        Ok(at.0)
    }

    /// Snapshot consistente + cursor.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.catalog.snapshot()
    }

    /// Eventos contíguos desde o cursor (G-49).
    pub fn events_since(&self, since: u64) -> Result<Vec<Event>, AuthorityError> {
        self.catalog
            .events_since(Cursor(since))
            .map_err(AuthorityError::from_catalog)
    }
}

impl Default for CatalogAuthority {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_journal(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "studio-catalog-{name}-{}.jsonl",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn stale_base_is_rejected_before_effects() {
        let mut authority = CatalogAuthority::new();
        let rev = authority
            .publish(String::from("d"), None, String::from("v1"), 0)
            .expect("cria");

        let err = authority
            .publish(String::from("d"), Some(rev + 9), String::from("v2"), 0)
            .expect_err("base obsoleta deve falhar");

        assert!(matches!(err, AuthorityError::Conflict { current: Some(_) }));
        assert_eq!(authority.snapshot().items.len(), 1);
    }

    #[test]
    fn journal_rebuilds_across_restart() {
        let path = temp_journal("replay");
        let mut first = CatalogAuthority::with_journal(path.clone()).expect("boot novo");
        first
            .publish(String::from("d"), None, String::from("v1"), 0)
            .expect("publica");
        first
            .delete(String::from("d"), 0)
            .expect("exclui com base 0");

        let second = CatalogAuthority::with_journal(path.clone()).expect("reboot reconstroi");
        let _ = std::fs::remove_file(&path);

        assert!(second.snapshot().items.is_empty());
        let replayed = second.events_since(0).expect("cursor 0 valido");
        assert_eq!(replayed.len(), 2);
    }

    #[test]
    fn corrupt_journal_fails_fast() {
        let path = temp_journal("corrompido");
        std::fs::write(&path, "não é json\n").expect("fixture");
        let err = CatalogAuthority::with_journal(path.clone()).expect_err("deve falhar");
        let _ = std::fs::remove_file(&path);

        assert!(matches!(err, AuthorityError::Corrupt { .. }));
    }
}
