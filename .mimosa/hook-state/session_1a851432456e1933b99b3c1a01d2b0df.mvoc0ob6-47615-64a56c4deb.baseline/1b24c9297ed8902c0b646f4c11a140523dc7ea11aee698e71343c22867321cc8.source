//! Journal SQLite do engine (T-900-M5-01; REQ-900-008/010; ADR-004/D-JOURNAL).
//!
//! Autoridade LOCAL do engine ÚNICO (sem HA V1 — R-SINGLE): decisões aceitas,
//! orçamento/reservas, tentativas e outbox transacional. Invariantes:
//! - reserva é TRANSACIONAL (constraint no banco: reserved+consumed <= teto);
//! - replay da mesma reserva (chave idempotente) NÃO reserva duas vezes;
//! - consumo incerto (timeout/crash) cobra a reserva INTEGRAL;
//! - outbox na MESMA transação da mudança de estado (publicação fora do tx);
//! - reabrir após crash conserva a decisão aceita ou falha fechada (corrupção
//!   → erro tipado, nunca estado silenciosamente perdido).

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("outra instância do engine mantém o lock: {0}")]
    AlreadyLocked(String),
    #[error("orçamento insuficiente: reservado+pedido {requested} > teto {budget} ({scope})")]
    InsufficientBudget {
        scope: String,
        requested: u64,
        budget: u64,
    },
    #[error("journal corrompido (migration/estado inválido): {0}")]
    Corrupted(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, JournalError>;

const SCHEMA_VERSION: i64 = 2;

const MIGRATION_V1: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS intents (
    workflow_id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    prompt TEXT NOT NULL,
    created_at_ns INTEGER NOT NULL,
    deadline_ns INTEGER NOT NULL,
    max_tokens INTEGER NOT NULL,
    max_cost_microunits INTEGER NOT NULL,
    intent_revision INTEGER NOT NULL,
    questions_revision INTEGER NOT NULL,
    policy_revision INTEGER NOT NULL,
    capability_catalog_revision INTEGER NOT NULL,
    checkpoint_revision TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS decisions (
    workflow_id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    decision_id TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    effective INTEGER NOT NULL,          -- 1 = decisão efetiva vigente
    verdict TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS workflows (
    workflow_id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    budget_microunits INTEGER NOT NULL,  -- teto agregado (contracts)
    budget_reserved INTEGER NOT NULL DEFAULT 0 CHECK (budget_reserved + budget_consumed <= budget_microunits),
    budget_consumed INTEGER NOT NULL DEFAULT 0,
    max_tokens INTEGER NOT NULL,
    status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS reservations (
    idempotency_key TEXT PRIMARY KEY,    -- (chamada/tentativa): replay não reserva 2x
    workflow_id TEXT NOT NULL REFERENCES workflows(workflow_id),
    purpose TEXT NOT NULL,               -- 'planner' | 'attempt:<node>:<n>'
    tokens_reserved INTEGER NOT NULL,
    microunits_reserved INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('reserved','consumed_measured','consumed_full')),
    created_ns INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS attempts (
    workflow_id TEXT NOT NULL REFERENCES workflows(workflow_id),
    node_id TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    task_id TEXT NOT NULL UNIQUE,        -- laya1-<sha> físico por tentativa
    state TEXT NOT NULL CHECK (state IN ('pending','published','in_flight','in_doubt','done','failed','cancelled')),
    PRIMARY KEY (workflow_id, node_id, attempt)
);
CREATE TABLE IF NOT EXISTS outbox (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id TEXT NOT NULL,
    kind TEXT NOT NULL,                  -- 'publish_task' | 'publish_state' | ...
    payload TEXT NOT NULL,
    created_ns INTEGER NOT NULL,
    published_ns INTEGER                 -- NULL = pendente
);
CREATE INDEX IF NOT EXISTS idx_outbox_pending ON outbox(published_ns) WHERE published_ns IS NULL;
"#;

/// Intent persistida (restart recovery do runtime).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredIntent {
    pub workflow_id: String,
    pub request_id: String,
    pub prompt: String,
    pub created_at_ns: u64,
    pub deadline_ns: u64,
    pub max_tokens: u64,
    pub max_cost_microunits: u64,
    pub intent_revision: u32,
    pub questions_revision: u32,
    pub policy_revision: u32,
    pub capability_catalog_revision: u32,
    pub checkpoint_revision: String,
}

/// Handle do journal com lock de instância (arquivo exclusivo; drop libera).
pub struct Journal {
    conn: rusqlite::Connection,
    _lock: InstanceLock,
}

struct InstanceLock {
    path: std::path::PathBuf,
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Journal {
    /// Abre (ou cria) o journal em `dir` com lock de instância e migrations.
    ///
    /// Corrupção (schema version desconhecida/arquivo inválido) → erro
    /// tipado (fail-closed), nunca recria silenciosamente.
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let lock_path = dir.join("engine.lock");
        if lock_path.exists() {
            // Lock órfão de crash: um lockfile com PID morto é removível.
            let pid = std::fs::read_to_string(&lock_path).unwrap_or_default();
            if let Ok(p) = pid.trim().parse::<i32>() {
                if !proc_alive(p) {
                    std::fs::remove_file(&lock_path).ok();
                }
            }
        }
        let mut lock_file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path)
            .map_err(|e| JournalError::AlreadyLocked(format!("{}: {e}", lock_path.display())))?;
        use std::io::Write;
        let _ = lock_file
            .write_all(std::process::id().to_string().as_bytes())
            .and_then(|_| lock_file.sync_all());
        drop(lock_file);

        let conn = rusqlite::Connection::open(dir.join("engine.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn,
            _lock: InstanceLock { path: lock_path },
        })
    }

    fn migrate(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute_batch(MIGRATION_V1)?;
        let version: i64 = conn.query_row(
            "SELECT COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key='schema_version'), 0)",
            [],
            |r| r.get(0),
        )?;
        match version {
            0 => {
                conn.execute(
                    "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)",
                    [SCHEMA_VERSION.to_string()],
                )?;
                Ok(())
            }
            v if v == SCHEMA_VERSION => Ok(()),
            1 => {
                // v1 -> v2: intents é criada pelo CREATE IF NOT EXISTS acima
                conn.execute(
                    "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [SCHEMA_VERSION.to_string()],
                )?;
                Ok(())
            }
            v => Err(JournalError::Corrupted(format!(
                "schema_version {v} != suportada {SCHEMA_VERSION}"
            ))),
        }
    }

    /// Registra workflow + decisão EFETIVA (slot 1 por workflow: segunda
    /// decisão com payload diferente é CONFLITO, igual é replay idempotente).
    #[allow(clippy::too_many_arguments)] // campos distintos do registro de admissão
    pub fn admit_workflow(
        &self,
        workflow_id: &str,
        request_id: &str,
        decision_id: &str,
        payload_hash: &str,
        verdict: &str,
        budget_microunits: u64,
        max_tokens: u64,
        prompt: &str,
        created_at_ns: u64,
        deadline_ns: u64,
        intent_revision: u32,
        questions_revision: u32,
        policy_revision: u32,
        capability_catalog_revision: u32,
        checkpoint_revision: &str,
    ) -> Result<AdmitOutcome> {
        let exists: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT payload_hash, effective FROM decisions WHERE workflow_id = ?1",
                [workflow_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(JournalError::from(other)),
            })?;
        if let Some((hash, _)) = exists {
            return Ok(if hash == payload_hash {
                AdmitOutcome::Replay
            } else {
                AdmitOutcome::Conflict
            });
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO decisions(workflow_id, request_id, decision_id, payload_hash, effective, verdict)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            rusqlite::params![workflow_id, request_id, decision_id, payload_hash, verdict],
        )?;
        tx.execute(
            "INSERT INTO workflows(workflow_id, request_id, budget_microunits, max_tokens, status)
             VALUES (?1, ?2, ?3, ?4, 'accepted')",
            rusqlite::params![
                workflow_id,
                request_id,
                budget_microunits as i64,
                max_tokens as i64
            ],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO intents(workflow_id, request_id, prompt, created_at_ns, deadline_ns,
                 max_tokens, max_cost_microunits, intent_revision, questions_revision,
                 policy_revision, capability_catalog_revision, checkpoint_revision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                workflow_id,
                request_id,
                prompt,
                created_at_ns as i64,
                deadline_ns as i64,
                max_tokens as i64,
                budget_microunits as i64,
                intent_revision,
                questions_revision,
                policy_revision,
                capability_catalog_revision,
                checkpoint_revision
            ],
        )?;
        tx.commit()?;
        Ok(AdmitOutcome::Admitted)
    }

    /// Reserva TRANSACIONAL por chave idempotente: replay retorna a reserva
    /// original (não duplica). Falha de constraint = orçamento insuficiente.
    pub fn reserve(
        &self,
        workflow_id: &str,
        idempotency_key: &str,
        purpose: &str,
        tokens: u64,
        microunits: u64,
    ) -> Result<ReserveOutcome> {
        let existing: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT tokens_reserved, state FROM reservations WHERE idempotency_key = ?1",
                [idempotency_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(JournalError::from(other)),
            })?;
        if let Some((tok, state)) = existing {
            // Replay: reserva ativa NÃO é renovada; consumida é devolvida
            // como AlreadyConsumed (o chamador não re-executa).
            return Ok(match state.as_str() {
                "reserved" => ReserveOutcome::Replayed { tokens: tok as u64 },
                _ => ReserveOutcome::AlreadyConsumed,
            });
        }
        let tx = self.conn.unchecked_transaction()?;
        let budget: i64 = tx.query_row(
            "SELECT budget_microunits - budget_reserved - budget_consumed FROM workflows WHERE workflow_id = ?1",
            [workflow_id],
            |r| r.get(0),
        )?;
        if budget < microunits as i64 {
            return Err(JournalError::InsufficientBudget {
                scope: purpose.to_string(),
                requested: microunits,
                budget: budget.unsigned_abs(),
            });
        }
        tx.execute(
            "UPDATE workflows SET budget_reserved = budget_reserved + ?1 WHERE workflow_id = ?2",
            rusqlite::params![microunits as i64, workflow_id],
        )?;
        tx.execute(
            "INSERT INTO reservations(idempotency_key, workflow_id, purpose, tokens_reserved,
                                      microunits_reserved, state, created_ns)
             VALUES (?1, ?2, ?3, ?4, ?5, 'reserved', ?6)",
            rusqlite::params![
                idempotency_key,
                workflow_id,
                purpose,
                tokens as i64,
                microunits as i64,
                0i64
            ],
        )?;
        tx.commit()?;
        Ok(ReserveOutcome::Reserved { tokens, microunits })
    }

    /// Libera reserva: `measured = None` (incerto) cobra a reserva INTEGRAL
    /// (contracts); medido cobra o medido e devolve o resto.
    pub fn release_reservation(
        &self,
        idempotency_key: &str,
        measured_microunits: Option<u64>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let reserved: i64 = tx.query_row(
            "SELECT microunits_reserved FROM reservations WHERE idempotency_key = ?1 AND state = 'reserved'",
            [idempotency_key],
            |r| r.get(0),
        )?;
        let charged = measured_microunits.map(|m| m as i64).unwrap_or(reserved);
        tx.execute(
            "UPDATE reservations SET state = ?1 WHERE idempotency_key = ?2",
            rusqlite::params![
                if measured_microunits.is_some() {
                    "consumed_measured"
                } else {
                    "consumed_full"
                },
                idempotency_key
            ],
        )?;
        tx.execute(
            "UPDATE workflows SET budget_reserved = budget_reserved - ?1,
                                  budget_consumed = budget_consumed + ?2
             WHERE workflow_id = (SELECT workflow_id FROM reservations WHERE idempotency_key = ?3)",
            rusqlite::params![reserved, charged, idempotency_key],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Registra tentativa com task_id físico (idempotente por PK composta).
    pub fn record_attempt(
        &self,
        workflow_id: &str,
        node_id: &str,
        attempt: u32,
        task_id: &str,
        state: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO attempts(workflow_id, node_id, attempt, task_id, state)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workflow_id, node_id, attempt) DO UPDATE SET state = excluded.state
             WHERE attempts.task_id = excluded.task_id",
            rusqlite::params![workflow_id, node_id, attempt, task_id, state],
        )?;
        Ok(())
    }

    /// Intents admitidas (rehidratação pós-restart).
    pub fn stored_intents(&self) -> Result<Vec<StoredIntent>> {
        let mut stmt = self.conn.prepare(
            "SELECT workflow_id, request_id, prompt, created_at_ns, deadline_ns, max_tokens,
                    max_cost_microunits, intent_revision, questions_revision, policy_revision,
                    capability_catalog_revision, checkpoint_revision FROM intents",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(StoredIntent {
                workflow_id: r.get(0)?,
                request_id: r.get(1)?,
                prompt: r.get(2)?,
                created_at_ns: r.get::<_, i64>(3)? as u64,
                deadline_ns: r.get::<_, i64>(4)? as u64,
                max_tokens: r.get::<_, i64>(5)? as u64,
                max_cost_microunits: r.get::<_, i64>(6)? as u64,
                intent_revision: r.get::<_, i64>(7)? as u32,
                questions_revision: r.get::<_, i64>(8)? as u32,
                policy_revision: r.get::<_, i64>(9)? as u32,
                capability_catalog_revision: r.get::<_, i64>(10)? as u32,
                checkpoint_revision: r.get(11)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Estado da tentativa (recovery).
    pub fn attempt_state(&self, workflow_id: &str, node_id: &str, attempt: u32) -> Result<String> {
        Ok(self.conn.query_row(
            "SELECT state FROM attempts WHERE workflow_id = ?1 AND node_id = ?2 AND attempt = ?3",
            rusqlite::params![workflow_id, node_id, attempt],
            |r| r.get(0),
        )?)
    }

    /// Marca tentativa IN_DOUBT (efeito externo incerto — não reexecutar
    /// automaticamente; M5-04).
    pub fn mark_in_doubt(&self, task_id: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE attempts SET state = 'in_doubt' WHERE task_id = ?1",
            [task_id],
        )?;
        anyhow_ok(n)
    }

    /// Tentativas IN_DOUBT pendentes (reconciliação manual/humano no gate).
    pub fn in_doubt_attempts(&self) -> Result<Vec<(String, String, u32, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT workflow_id, node_id, attempt, task_id FROM attempts WHERE state = 'in_doubt'",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u32, r.get(3)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Append no outbox na MESMA transação de uma mudança de estado.
    pub fn outbox_append(
        &self,
        tx_step: &dyn Fn(&rusqlite::Transaction<'_>) -> Result<()>,
        workflow_id: &str,
        kind: &str,
        payload: &str,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx_step(&tx)?;
        tx.execute(
            "INSERT INTO outbox(workflow_id, kind, payload, created_ns) VALUES (?1, ?2, ?3, 0)",
            rusqlite::params![workflow_id, kind, payload],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Drena o outbox pendente (publicação for a tx; marca published).
    pub fn outbox_drain(&self, mut publish: impl FnMut(&str, &str) -> Result<()>) -> Result<usize> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, kind, payload FROM outbox WHERE published_ns IS NULL ORDER BY seq",
        )?;
        let entries: Vec<(i64, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut published = 0;
        for (seq, kind, payload) in entries {
            publish(&kind, &payload)?;
            self.conn
                .execute("UPDATE outbox SET published_ns = 1 WHERE seq = ?1", [seq])?;
            published += 1;
        }
        Ok(published)
    }

    /// Estado do orçamento (observabilidade/limite).
    pub fn budget_status(&self, workflow_id: &str) -> Result<(u64, u64, u64)> {
        let r = self.conn.query_row(
            "SELECT budget_microunits, budget_reserved, budget_consumed FROM workflows WHERE workflow_id = ?1",
            [workflow_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get::<_, i64>(1)? as u64,
                    r.get::<_, i64>(2)? as u64,
                ))
            },
        )?;
        Ok(r)
    }
}

fn anyhow_ok(_n: usize) -> Result<()> {
    Ok(())
}

fn proc_alive(pid: i32) -> bool {
    // kill(pid, 0) no próprio processo é permitido sem permissão especial.
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitOutcome {
    Admitted,
    Replay,
    Conflict,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReserveOutcome {
    Reserved {
        tokens: u64,
        microunits: u64,
    },
    /// Replay da MESMA chave antes da liberação: usa a reserva existente.
    Replayed {
        tokens: u64,
    },
    /// Já consumida: o chamador NÃO re-executa o trabalho cobrado.
    AlreadyConsumed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wf-journal-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn admit_is_idempotent_and_conflict_typed() {
        let dir = tmp("admit");
        let j = Journal::open(&dir).unwrap();
        let out = j
            .admit_workflow(
                "wf", "req", "dec", "hash-a", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, "",
            )
            .unwrap();
        assert_eq!(out, AdmitOutcome::Admitted);
        // replay igual
        assert_eq!(
            j.admit_workflow(
                "wf", "req", "dec", "hash-a", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, ""
            )
            .unwrap(),
            AdmitOutcome::Replay
        );
        // payload diferente = conflito
        assert_eq!(
            j.admit_workflow(
                "wf", "req", "dec2", "hash-b", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, ""
            )
            .unwrap(),
            AdmitOutcome::Conflict
        );
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reopen_after_drop_keeps_decisions_and_lock_is_reusable() {
        let dir = tmp("reopen");
        {
            let j = Journal::open(&dir).unwrap();
            j.admit_workflow(
                "wf", "r", "d", "h", "accept", 500, 50, "prompt", 1, 9, 1, 1, 1, 1, "",
            )
            .unwrap();
            // segunda instância WHILE lock ativo → AlreadyLocked
            assert!(matches!(
                Journal::open(&dir),
                Err(JournalError::AlreadyLocked(_))
            ));
        } // drop libera o lock
        let j2 = Journal::open(&dir).unwrap();
        assert_eq!(
            j2.admit_workflow(
                "wf", "r", "d", "h", "accept", 500, 50, "prompt", 1, 9, 1, 1, 1, 1, ""
            )
            .unwrap(),
            AdmitOutcome::Replay,
            "crash/restart conserva a decisão aceita"
        );
        drop(j2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn orphan_lock_of_dead_process_is_reclaimed() {
        let dir = tmp("orphan");
        std::fs::create_dir_all(&dir).unwrap();
        // PID certamente morto (init não é nosso processo; usa pid enorme)
        std::fs::write(dir.join("engine.lock"), "99999999").unwrap();
        let j =
            Journal::open(&dir).unwrap_or_else(|e| panic!("lock órfão deveria ser reciclado: {e}"));
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reserve_is_transactional_and_replay_safe() {
        let dir = tmp("reserve");
        let j = Journal::open(&dir).unwrap();
        j.admit_workflow(
            "wf", "r", "d", "h", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, "",
        )
        .unwrap();

        let r1 = j.reserve("wf", "call-1", "planner", 50, 400).unwrap();
        assert!(matches!(
            r1,
            ReserveOutcome::Reserved {
                microunits: 400,
                ..
            }
        ));
        // REPLAY da mesma chave: não duplica (mesma reserva)
        assert!(matches!(
            j.reserve("wf", "call-1", "planner", 50, 400).unwrap(),
            ReserveOutcome::Replayed { tokens: 50 }
        ));
        assert_eq!(j.budget_status("wf").unwrap(), (1000, 400, 0));

        // acima do teto → InsufficientBudget (600 disponível, pede 700)
        assert!(matches!(
            j.reserve("wf", "call-2", "attempt:a:1", 10, 700),
            Err(JournalError::InsufficientBudget { .. })
        ));
        // nada foi reservado a mais na falha
        assert_eq!(j.budget_status("wf").unwrap(), (1000, 400, 0));
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uncertain_consumption_charges_full_reservation() {
        let dir = tmp("uncertain");
        let j = Journal::open(&dir).unwrap();
        j.admit_workflow(
            "wf", "r", "d", "h", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, "",
        )
        .unwrap();
        j.reserve("wf", "call-9", "attempt:x:1", 80, 300).unwrap();
        // timeout/crash: consumo incerto → reserva INTEGRAL cobrada
        j.release_reservation("call-9", None).unwrap();
        let (total, reserved, consumed) = j.budget_status("wf").unwrap();
        assert_eq!((total, reserved, consumed), (1000, 0, 300));
        // medido: só o medido é cobrado, o resto volta
        j.reserve("wf", "call-10", "attempt:y:1", 80, 300).unwrap();
        j.release_reservation("call-10", Some(120)).unwrap();
        assert_eq!(j.budget_status("wf").unwrap(), (1000, 0, 420));
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn outbox_is_atomic_with_state_and_replay_publishes_pending_only() {
        let dir = tmp("outbox");
        let j = Journal::open(&dir).unwrap();
        j.admit_workflow(
            "wf", "r", "d", "h", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, "",
        )
        .unwrap();
        j.outbox_append(
            &|tx| {
                tx.execute(
                    "UPDATE workflows SET status = 'running' WHERE workflow_id = 'wf'",
                    [],
                )?;
                Ok(())
            },
            "wf",
            "publish_state",
            "{\"s\":\"running\"}",
        )
        .unwrap();
        let status: String = j
            .conn
            .query_row(
                "SELECT status FROM workflows WHERE workflow_id = 'wf'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "running", "estado e outbox na MESMA transação");

        let mut seen = Vec::new();
        let n = j
            .outbox_drain(|kind, payload| {
                seen.push((kind.to_string(), payload.to_string()));
                Ok(())
            })
            .unwrap();
        assert_eq!(n, 1);
        // replay do drain: nada pendente
        let n2 = j
            .outbox_drain(|_, _| panic!("nada deveria estar pendente"))
            .unwrap();
        assert_eq!(n2, 0);
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn in_doubt_attempts_survive_restart_and_are_listed() {
        let dir = tmp("indoubt");
        {
            let j = Journal::open(&dir).unwrap();
            j.admit_workflow(
                "wf", "r", "d", "h", "accept", 1000, 100, "prompt", 1, 9, 1, 1, 1, 1, "",
            )
            .unwrap();
            j.record_attempt("wf", "n1", 1, "laya1-abc", "in_flight")
                .unwrap();
            j.mark_in_doubt("laya1-abc").unwrap();
        }
        let j2 = Journal::open(&dir).unwrap();
        let pend = j2.in_doubt_attempts().unwrap();
        assert_eq!(pend.len(), 1);
        assert_eq!(pend[0].3, "laya1-abc");
        drop(j2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_db_fails_closed() {
        let dir = tmp("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        // arquivo DB que não é sqlite
        std::fs::write(dir.join("engine.db"), b"isto nao e sqlite").unwrap();
        assert!(matches!(
            Journal::open(&dir),
            Err(JournalError::Sqlite(_)) | Err(JournalError::Corrupted(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
