//! Event sink que grava eventos em arquivo JSONL.
//!
//! Porte de `src/orchestrator/observability/file_sink.py`. O `Protocol
//! EventSink` do Python vira a trait [`EventSink`]; o `threading.Lock` vira
//! `Mutex` e os contadores de operação são atômicos (o Python não os tinha).

use crate::events::{EventType, ObservabilityEvent};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Erros do sink (thiserror — convenção das libs do workspace).
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// Falha de I/O ao abrir/gravar/ler o arquivo JSONL.
    #[error("falha de I/O em {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Falha ao serializar/desserializar um evento.
    #[error("falha JSON no sink: {0}")]
    Serde(#[from] serde_json::Error),
    /// Mutex do buffer envenenado (panico com lock segurado).
    #[error("mutex do sink envenenado")]
    Poisoned,
}

/// Contrato de sink de eventos (porte do `Protocol EventSink` do Python).
pub trait EventSink: Send + Sync {
    /// Enfileira o evento para gravação (flush automático a cada N eventos).
    fn emit(&self, event: &ObservabilityEvent) -> Result<(), SinkError>;
    /// Consulta eventos já gravados, filtrando por `task_id` (vazio = todos)
    /// e/ou `event_type`, até `limit` resultados em ordem de gravação.
    fn query(
        &self,
        task_id: &str,
        event_type: Option<EventType>,
        limit: usize,
    ) -> Result<Vec<ObservabilityEvent>, SinkError>;
    /// Força a gravação do buffer pendente.
    fn flush(&self) -> Result<(), SinkError>;
}

/// Intervalo de flush default (paridade com `_flush_interval = 50` do Python).
pub const DEFAULT_FLUSH_INTERVAL: usize = 50;

/// Sink que grava eventos em arquivo JSONL (append) para análise posterior.
///
/// Porte de `FileEventSink`: buffer em memória com flush a cada
/// `flush_interval` eventos (e em `query`/`flush` explícitos). O arquivo é
/// aberto a cada flush (paridade com o Python; seguro em FS de rede/SMB).
pub struct FileEventSink {
    path: PathBuf,
    buffer: Mutex<Vec<ObservabilityEvent>>,
    flush_interval: usize,
    /// Eventos aceitos em `emit`.
    emitted: AtomicU64,
    /// Eventos efetivamente gravados em disco.
    flushed: AtomicU64,
    /// Falhas de escrita (o Python só logava; aqui também propagamos o erro).
    write_errors: AtomicU64,
}

impl FileEventSink {
    /// Cria o sink em `path` (default do Python: `/tmp/dds_observability.jsonl`).
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SinkError> {
        Self::with_flush_interval(path, DEFAULT_FLUSH_INTERVAL)
    }

    /// Cria o sink com intervalo de flush customizado.
    pub fn with_flush_interval(
        path: impl AsRef<Path>,
        flush_interval: usize,
    ) -> Result<Self, SinkError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| SinkError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        tracing::info!(path = %path.display(), "FileEventSink inicializado");
        Ok(Self {
            path,
            buffer: Mutex::new(Vec::new()),
            flush_interval,
            emitted: AtomicU64::new(0),
            flushed: AtomicU64::new(0),
            write_errors: AtomicU64::new(0),
        })
    }

    /// Caminho do arquivo JSONL.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Eventos aceitos em `emit` (contador atômico).
    pub fn emitted_count(&self) -> u64 {
        self.emitted.load(Ordering::Relaxed)
    }

    /// Eventos gravados em disco (contador atômico).
    pub fn flushed_count(&self) -> u64 {
        self.flushed.load(Ordering::Relaxed)
    }

    /// Falhas de escrita (contador atômico).
    pub fn write_error_count(&self) -> u64 {
        self.write_errors.load(Ordering::Relaxed)
    }

    /// Grava o buffer no arquivo. Chamador deve segurar o lock do buffer.
    fn flush_locked(&self, buffer: &mut Vec<ObservabilityEvent>) -> Result<(), SinkError> {
        if buffer.is_empty() {
            return Ok(());
        }
        let result = (|| {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|source| SinkError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            for event in buffer.iter() {
                let line = serde_json::to_string(event)?;
                writeln!(file, "{line}").map_err(|source| SinkError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            }
            Ok(buffer.len() as u64)
        })();
        match result {
            Ok(written) => {
                buffer.clear();
                self.flushed.fetch_add(written, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                self.write_errors.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }
}

impl EventSink for FileEventSink {
    fn emit(&self, event: &ObservabilityEvent) -> Result<(), SinkError> {
        self.emitted.fetch_add(1, Ordering::Relaxed);
        let mut buffer = self.buffer.lock().map_err(|_| SinkError::Poisoned)?;
        buffer.push(event.clone());
        if buffer.len() >= self.flush_interval {
            self.flush_locked(&mut buffer)?;
        }
        Ok(())
    }

    fn query(
        &self,
        task_id: &str,
        event_type: Option<EventType>,
        limit: usize,
    ) -> Result<Vec<ObservabilityEvent>, SinkError> {
        // Paridade com o Python: query faz flush antes de ler.
        self.flush()?;
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(SinkError::Io {
                    path: self.path.clone(),
                    source,
                })
            }
        };
        let mut results = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|source| SinkError::Io {
                path: self.path.clone(),
                source,
            })?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Robusto a linhas truncadas (crash no meio do append): o Python
            // quebrava com exceção; aqui pulamos e contamos no log.
            let event: ObservabilityEvent = match serde_json::from_str(line) {
                Ok(ev) => ev,
                Err(e) => {
                    tracing::warn!(error = %e, path = %self.path.display(), "linha JSONL inválida ignorada");
                    continue;
                }
            };
            if !task_id.is_empty() && event.task_id != task_id {
                continue;
            }
            if let Some(ty) = event_type {
                if event.event_type != ty {
                    continue;
                }
            }
            results.push(event);
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }

    fn flush(&self) -> Result<(), SinkError> {
        let mut buffer = self.buffer.lock().map_err(|_| SinkError::Poisoned)?;
        self.flush_locked(&mut buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::now_ns;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "observability_sink_{tag}_{}_{}",
            std::process::id(),
            now_ns()
        ));
        std::fs::create_dir_all(&dir).expect("criar temp dir");
        dir
    }

    fn event(task_id: &str, ty: EventType) -> ObservabilityEvent {
        let mut ev = ObservabilityEvent::new(ty);
        ev.task_id = task_id.into();
        ev
    }

    #[test]
    fn emit_buffers_until_flush_interval() {
        let dir = temp_dir("buffer");
        let path = dir.join("events.jsonl");
        let sink = FileEventSink::with_flush_interval(&path, 50).expect("sink");

        for _ in 0..49 {
            sink.emit(&event("t1", EventType::RequestReceived))
                .expect("emit");
        }
        // Ainda bufferizado: arquivo nem foi criado.
        assert!(!path.exists());
        assert_eq!(sink.emitted_count(), 49);
        assert_eq!(sink.flushed_count(), 0);

        sink.emit(&event("t1", EventType::RequestReceived))
            .expect("emit 50");
        assert_eq!(sink.flushed_count(), 50);
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content.lines().count(), 50);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn explicit_flush_writes_pending_buffer() {
        let dir = temp_dir("flush");
        let path = dir.join("events.jsonl");
        let sink = FileEventSink::new(&path).expect("sink");
        sink.emit(&event("t1", EventType::Error)).expect("emit");
        sink.flush().expect("flush");
        assert_eq!(sink.flushed_count(), 1);
        let line = std::fs::read_to_string(&path).expect("read");
        assert!(line.contains("\"event_type\":\"ERROR\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn query_filters_by_task_type_and_limit() {
        let dir = temp_dir("query");
        let path = dir.join("events.jsonl");
        let sink = FileEventSink::new(&path).expect("sink");
        for i in 0..10 {
            let ty = if i % 2 == 0 {
                EventType::LlmResultReceived
            } else {
                EventType::Error
            };
            sink.emit(&event(&format!("task-{}", i % 3), ty))
                .expect("emit");
        }
        // query faz flush implícito.
        let all = sink.query("", None, 100).expect("query all");
        assert_eq!(all.len(), 10);

        let by_task = sink.query("task-1", None, 100).expect("query task");
        assert!(by_task.iter().all(|e| e.task_id == "task-1"));
        // i % 3 == 1 for i in 0..10 → i = 1, 4, 7 → 3 items
        assert_eq!(by_task.len(), 3);
    }
}
