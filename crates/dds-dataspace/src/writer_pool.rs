//! Pool de writers MPMC com backpressure (T-305, REQ-305).
//!
//! Substitui a thread única de escrita do Python (`_write_queue`, maxsize 10k,
//! 1 `dds-write-loop`): K workers drenam um canal `crossbeam` bounded e escrevem
//! no DDS em paralelo real (sem GIL). `DataWriter` do CycloneDDS é thread-safe
//! para `write` concorrente no mesmo writer.
//!
//! **Política de backpressure (documentada):** canal bounded; quando cheio,
//! `submit` falha rápido com `WriteFailed("backpressure: fila cheia")` — o
//! chamador decide (retry com backoff / coalescer / dropar). Nunca bloqueia o
//! hot path de quem produz (ex.: stream de inferência do agente).

use crate::api::DataSpaceError;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use cyclonedds::DataWriter;
use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task, TaskOutput};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Pedido de escrita genérico.
pub enum WriteRequest {
    Task(Task),
    Agent(AgentState),
    Output(TaskOutput),
}

type WriteFn = Arc<dyn Fn(WriteRequest) + Send + Sync>;

/// Pool de workers de escrita.
pub struct WriterPool {
    tx: Sender<WriteRequest>,
    workers: Vec<std::thread::JoinHandle<()>>,
    submitted: Arc<AtomicU64>,
    completed: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
}

impl WriterPool {
    /// Cria o pool com `n_workers` drenando um canal bounded de `capacity`.
    /// `write_fn` recebe o pedido e escreve no DDS (closure sobre os DataWriters).
    pub fn new(n_workers: usize, capacity: usize, write_fn: WriteFn) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(capacity);
        let submitted = Arc::new(AtomicU64::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));

        let workers = (0..n_workers)
            .map(|i| {
                let rx: Receiver<WriteRequest> = rx.clone();
                let write_fn = Arc::clone(&write_fn);
                let completed = Arc::clone(&completed);
                let failed = Arc::clone(&failed);
                std::thread::Builder::new()
                    .name(format!("dds-writer-{i}"))
                    .spawn(move || {
                        while let Ok(req) = rx.recv() {
                            write_fn(req);
                            completed.fetch_add(1, Ordering::Relaxed);
                        }
                        // canal fechado + drenado → sai
                        let _ = failed;
                    })
                    .expect("spawn dds-writer")
            })
            .collect();

        Self {
            tx,
            workers,
            submitted,
            completed,
            failed,
        }
    }

    /// Enfileira uma escrita. Falha rápido se a fila estiver cheia (backpressure).
    pub fn submit(&self, req: WriteRequest) -> Result<(), DataSpaceError> {
        self.submitted.fetch_add(1, Ordering::Relaxed);
        self.tx.try_send(req).map_err(|e| match e {
            TrySendError::Full(_) => {
                self.failed.fetch_add(1, Ordering::Relaxed);
                DataSpaceError::WriteFailed("backpressure: fila de escrita cheia".into())
            }
            TrySendError::Disconnected(_) => {
                self.failed.fetch_add(1, Ordering::Relaxed);
                DataSpaceError::WriteFailed("writer pool encerrado".into())
            }
        })
    }

    pub fn submitted(&self) -> u64 {
        self.submitted.load(Ordering::Relaxed)
    }
    pub fn completed(&self) -> u64 {
        self.completed.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    /// Fecha o canal e espera os workers drenarem.
    pub fn drain_and_shutdown(self) {
        drop(self.tx);
        for (i, w) in self.workers.into_iter().enumerate() {
            if let Err(e) = w.join() {
                tracing::error!(worker = i, error = ?e, "writer_pool: worker panou");
            }
        }
    }
}

/// Constrói a closure de escrita sobre os DataWriters do DataSpace
/// (DataWriter é handle copiável/thread-safe para write concorrente).
pub fn make_write_fn(
    tasks_writer: DataWriter<Task>,
    agents_writer: DataWriter<AgentState>,
    outputs_writer: DataWriter<TaskOutput>,
) -> WriteFn {
    Arc::new(move |req| {
        let result = match &req {
            WriteRequest::Task(t) => tasks_writer.write(t),
            WriteRequest::Agent(a) => agents_writer.write(a),
            WriteRequest::Output(o) => outputs_writer.write(o),
        };
        if let Err(e) = result {
            tracing::error!(error = %e, "writer_pool: falha ao escrever no DDS");
        }
    })
}
