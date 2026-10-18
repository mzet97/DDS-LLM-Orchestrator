//! Parte de `catalog_auth` (extraída para manter os arquivos revisáveis).

use super::*;

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
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| AuthorityError::Corrupt {
                path: path.display().to_string(),
                detail: err.to_string(),
            })?;
        let old_len =
            file.metadata()
                .map(|meta| meta.len())
                .map_err(|err| AuthorityError::Corrupt {
                    path: path.display().to_string(),
                    detail: err.to_string(),
                })?;
        Self::write_sync_truncate(&file, old_len, line.as_bytes(), std::fs::File::sync_all).map_err(
            |err| AuthorityError::Corrupt {
                path: path.display().to_string(),
                detail: err.to_string(),
            },
        )
    }

    /// Escreve + sincroniza; em falha de escrita/sync, trunca de volta ao
    /// `old_len` para a linha falha não ficar visível ao replay (P2: sem o
    /// truncate, o rebuild após `sync_all` falho avançava a revisão de uma
    /// mutação reportada como falha). `sync` injetável para teste direto.
    pub(super) fn write_sync_truncate(
        file: &std::fs::File,
        old_len: u64,
        bytes: &[u8],
        sync: impl FnOnce(&std::fs::File) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut sink: &std::fs::File = file;
        if let Err(err) = sink.write_all(bytes).and_then(|()| sync(file)) {
            let _ = file.set_len(old_len);
            return Err(err);
        }
        Ok(())
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
