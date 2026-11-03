//! Persistência do log de operações atrás de trait (T-890-05, G-34/36).
//!
//! Dois backends: o snapshot JSON (comportamento histórico) e o SQLite
//! relacional (default novo). Migração JSON→SQLite NÃO destrutiva: o arquivo
//! antigo é renomeado para `<nome>.imported` depois de importado com sucesso,
//! e a importação só hidrata estado — nenhuma operação é (re)executada.

use std::path::{Path, PathBuf};

use crate::operations::{NodeError, OpRecord, OperationLog};
use crate::protocol::OperationId;

/// Backend de persistência do [`OperationLog`].
pub trait Storage: Send + Sync {
    /// Persiste o estado completo (semântica de snapshot atômico).
    /// # Errors
    /// Falha de disco/serialização não mascara o estado em memória.
    fn save(&self, log: &OperationLog) -> Result<(), NodeError>;
    /// Carrega o estado persistido; `None` = primeira execução.
    /// # Errors
    /// Persistência corrompida falha rápido (não mascara com log vazio).
    fn load(&self) -> Result<Option<OperationLog>, NodeError>;
    /// Caminho do arquivo de persistência (base do journal do catálogo).
    fn path(&self) -> &Path;
}

/// Snapshot JSON (comportamento pré-T-890-05, mantido para compat).
#[derive(Debug)]
pub struct JsonStorage {
    path: PathBuf,
}

impl JsonStorage {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Storage for JsonStorage {
    fn save(&self, log: &OperationLog) -> Result<(), NodeError> {
        log.save(&self.path)
    }

    fn load(&self) -> Result<Option<OperationLog>, NodeError> {
        if !self.path.exists() {
            return Ok(None);
        }
        OperationLog::load(&self.path).map(Some)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

/// SQLite relacional (rusqlite bundled — sem dependência de sistema).
#[derive(Debug)]
pub struct SqliteStorage {
    path: PathBuf,
}

impl SqliteStorage {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn connection(&self) -> Result<rusqlite::Connection, NodeError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| NodeError::Storage(format!("{}: {err}", parent.display())))?;
        }
        let conn = rusqlite::Connection::open(&self.path)
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS log_meta (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS operations (
                 seq INTEGER PRIMARY KEY,
                 op_id TEXT NOT NULL UNIQUE,
                 op_json TEXT NOT NULL
             );",
        )
        .map_err(|err| NodeError::Storage(format!("sqlite schema: {err}")))?;
        Ok(conn)
    }

    /// Migração não destrutiva (G-34): se o banco está virgem e existe o
    /// snapshot JSON irmão (`<nome>.json`), importa e renomeia para
    /// `.imported`. Qualquer falha interrompe ANTES de tocar o JSON.
    fn migrate_from_json_if_fresh(&self, conn: &rusqlite::Connection) -> Result<(), NodeError> {
        let fresh = conn
            .query_row("SELECT COUNT(*) FROM log_meta", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?
            == 0
            && conn
                .query_row("SELECT COUNT(*) FROM operations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?
                == 0;
        if !fresh {
            return Ok(());
        }
        let json_path = self.path.with_extension("json");
        if !json_path.exists() {
            return Ok(());
        }
        let log = OperationLog::load(&json_path)?;
        self.write(conn, &log)?;
        let imported = json_path.with_extension("json.imported");
        std::fs::rename(&json_path, &imported)
            .map_err(|err| NodeError::Storage(format!("migração: {err}")))?;
        eprintln!(
            "studio-noded: journal JSON migrado para SQLite ({}) — original preservado como {}",
            self.path.display(),
            imported.display()
        );
        Ok(())
    }

    /// Escrita atômica: transação única, DELETE+INSERT (semântica de
    /// snapshot — idêntica ao backend JSON, agora com fsync do SQLite).
    fn write(&self, conn: &rusqlite::Connection, log: &OperationLog) -> Result<(), NodeError> {
        let owned = serde_json::to_string(log.owned_services())
            .map_err(|err| NodeError::Storage(err.to_string()))?;
        let mut stmt = conn
            .prepare("INSERT INTO operations (seq, op_id, op_json) VALUES (?1, ?2, ?3)")
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        let rows: Vec<(usize, &OperationId, String)> = log
            .order()
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let record = log.reconcile(id)?;
                serde_json::to_string(&record.op)
                    .ok()
                    .map(|op_json| (i + 1, id, op_json))
            })
            .collect();
        let tx = conn
            .unchecked_transaction()
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        tx.execute("DELETE FROM operations", [])
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        tx.execute("DELETE FROM log_meta", [])
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        tx.execute(
            "INSERT INTO log_meta (key, value) VALUES ('owned_services', ?1)",
            [&owned],
        )
        .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        for (seq, id, op_json) in &rows {
            stmt.execute(rusqlite::params![seq, id.0, op_json])
                .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        }
        tx.commit()
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        Ok(())
    }

    fn read(&self, conn: &rusqlite::Connection) -> Result<Option<OperationLog>, NodeError> {
        let owned: Option<String> = conn
            .query_row(
                "SELECT value FROM log_meta WHERE key = 'owned_services'",
                [],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(NodeError::Storage(format!("sqlite: {other}"))),
            })?;
        let Some(owned) = owned else {
            return Ok(None);
        };
        let owned_services: Vec<String> = serde_json::from_str(&owned)
            .map_err(|err| NodeError::Storage(format!("meta corrompida: {err}")))?;
        let mut stmt = conn
            .prepare("SELECT op_id, op_json FROM operations ORDER BY seq")
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
        let mut records = Vec::new();
        for row in rows {
            let (id, op_json) = row.map_err(|err| NodeError::Storage(format!("sqlite: {err}")))?;
            let op: crate::protocol::AdminOp = serde_json::from_str(&op_json)
                .map_err(|err| NodeError::Storage(format!("op corrompida {id}: {err}")))?;
            records.push(OpRecord {
                id: OperationId(id),
                op,
            });
        }
        Ok(Some(OperationLog::restore(owned_services, records)))
    }
}

impl Storage for SqliteStorage {
    fn save(&self, log: &OperationLog) -> Result<(), NodeError> {
        let conn = self.connection()?;
        self.write(&conn, log)
    }

    fn load(&self) -> Result<Option<OperationLog>, NodeError> {
        let conn = self.connection()?;
        self.migrate_from_json_if_fresh(&conn)?;
        self.read(&conn)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests;
