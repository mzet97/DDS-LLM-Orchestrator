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
        let mut authority = Self {
            catalog: Catalog::new(),
            journal: Some(path),
        };
        authority.rebuild_from_journal()?;
        Ok(authority)
    }

    /// Reconstrói a memória por replay do journal em disco (REQ/T-820-18):
    /// arquivo ausente = estado vazio (primeiro boot); linha inválida =
    /// `Corrupt` (fail rápido, operador corrige o arquivo).
    fn rebuild_from_journal(&mut self) -> Result<(), AuthorityError> {
        let Some(path) = self.journal.clone() else {
            return Ok(());
        };
        let path_str = path.display().to_string();
        let corrupt = |detail: String| AuthorityError::Corrupt {
            path: path_str.clone(),
            detail,
        };
        self.catalog = Catalog::new();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(corrupt(err.to_string())),
        };
        for (line_no, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(line)
                .map_err(|err| corrupt(format!("linha {line_no}: {err}")))?;
            self.replay(entry)
                .map_err(|detail| corrupt(format!("linha {line_no}: {detail}")))?;
        }
        Ok(())
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
    ///
    /// Contrato de durabilidade (REQ/T-820-18): a mutação só vale para o
    /// chamador se o journal aceitou. Em falha de append, a memória é
    /// reconstruída por replay do journal — o estado volta a ser EXATAMENTE
    /// o que está em disco (a mutação aplicada em memória é desfeita), e o
    /// erro de disco é propagado. Sem isso, falha de disco deixava memória
    /// e journal divergentes ("aceito, mas não sobrevive a reboot").
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
        let entry = JournalEntry::Publish {
            id,
            base,
            value,
            generation,
        };
        if let Err(err) = self.append(&entry) {
            self.rebuild_from_journal()?;
            return Err(err);
        }
        Ok(revision.0)
    }

    /// Exclui com tombstone e journaliza (G-57). Mesmo contrato de
    /// durabilidade de [`Self::publish`].
    pub fn delete(&mut self, id: String, base: u64) -> Result<u64, AuthorityError> {
        let at = self
            .catalog
            .delete(&DefinitionId(id.clone()), Revision(base))
            .map_err(AuthorityError::from_catalog)?;
        let entry = JournalEntry::Delete { id, base };
        if let Err(err) = self.append(&entry) {
            self.rebuild_from_journal()?;
            return Err(err);
        }
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

    /// REQ/T-820-18: falha de append desfaz a mutação em memória (replay do
    /// journal) — memória e disco nunca divergem; a mutação não aceita pelo
    /// disco não é reportada como aceita.
    #[test]
    fn append_failure_rolls_back_memory_to_journal_state() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_journal("rollback");
        let mut authority = CatalogAuthority::with_journal(path.clone()).expect("boot novo");
        authority
            .publish(String::from("d"), None, String::from("v1"), 0)
            .expect("primeira mutação journalizada");

        // Simula falha de disco: journal só-leitura (append falha, leitura ok).
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&path, perms).expect("chmod");

        let err = authority
            .publish(String::from("d2"), None, String::from("v2"), 0)
            .expect_err("append deve falhar");
        assert!(matches!(err, AuthorityError::Corrupt { .. }));

        // Rollback: memória == journal (d=v1 presente; d2 NÃO ficou aplicado).
        let snap = authority.snapshot();
        assert_eq!(snap.items.len(), 1, "apenas a mutação journalizada fica");
        assert_eq!(snap.items[0].id.0, "d");

        // Delete na mesma situação: mesmo contrato.
        let err = authority.delete(String::from("d"), 0).expect_err("falha");
        assert!(matches!(err, AuthorityError::Corrupt { .. }));
        assert_eq!(authority.snapshot().items.len(), 1, "rollback do delete");

        // Restaura permissões para limpar.
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&path, perms).expect("chmod");
        let _ = std::fs::remove_file(&path);
    }
}
