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
use cyclonedds::{DataWriter, DdsResult, DdsString, WriteLoan};
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
///
/// `tasks_writers` é um POOL (não um único writer): ver
/// `crate::select_task_writer_slot`/`crate::build_tasks_writer_pool` — o
/// mesmo mecanismo de força variada por slot usado no caminho de claim
/// principal (`DataSpace::write_task`), para que `WriteRequest::Task` nunca
/// reintroduza o desbalanceamento de carga entre agentes corrigido nesta
/// sessão, caso algum dia passe a ter um chamador em produção.
pub fn make_write_fn(
    tasks_writers: Vec<DataWriter<Task>>,
    agents_writer: DataWriter<AgentState>,
    outputs_writer: DataWriter<TaskOutput>,
) -> WriteFn {
    Arc::new(move |req| {
        let result = match &req {
            WriteRequest::Task(t) => {
                let idx = crate::select_task_writer_slot(&t.task_id, tasks_writers.len());
                tasks_writers[idx].write(t)
            }
            WriteRequest::Agent(a) => agents_writer.write(a),
            // Zero-copy: TaskOutput é o tópico de maior volume de samples (um
            // por chunk de streaming de inferência) — T-616. Ver
            // `write_output_loan` para o porquê do loan em vez de `.write()`.
            WriteRequest::Output(o) => write_output_loan(&outputs_writer, o),
        };
        if let Err(e) = result {
            tracing::error!(error = %e, "writer_pool: falha ao escrever no DDS");
        }
    })
}

/// Escreve um `TaskOutput` via loan zero-copy em vez de `.write()` (que
/// serializa para uma representação intermediária via `WriteArena` a cada
/// chamada). `TaskOutput` é o tópico de maior volume por sessão de inferência
/// (um sample por chunk de streaming) — o alvo certo para essa otimização.
///
/// Usa `DataWriter::request_loan`/`WriteLoan`, corrigido nesta sessão na
/// crate `cyclonedds` (ver `DdsType::Native` e o histórico no doc comment de
/// `request_loan` em `third_party/cyclonedds-rust/.../writer.rs`): antes da
/// correção, o loan zerava/interpretava o buffer como o tipo Rust ergonômico
/// (`TaskOutput`, com `String`), quando CycloneDDS na verdade aloca
/// `size_of::<TaskOutput::Native>()` bytes (menor, com `DdsString` de 8
/// bytes) — um estouro de buffer real, não só um risco teórico. Populamos os
/// 3 campos `String` como `DdsString` no tipo nativo; os demais campos são
/// primitivos e são copiados diretamente.
/// `pub` (não só de uso interno do `WriterPool`) para permitir o microbenchmark
/// `criterion` em `benches/write_loan.rs` (Fase R3) comparar diretamente contra
/// `DataWriter::write`.
pub fn write_output_loan(writer: &DataWriter<TaskOutput>, o: &TaskOutput) -> DdsResult<()> {
    let mut loan = writer.request_loan()?;
    let native = loan.get_mut();
    native.task_id = DdsString::new(&o.task_id)?;
    native.seq_num = o.seq_num;
    native.content = DdsString::new(&o.content)?;
    native.is_final = o.is_final;
    native.finish_reason = o.finish_reason;
    native.agent_id = DdsString::new(&o.agent_id)?;
    native.token_count = o.token_count;
    native.emitted_at_ns = o.emitted_at_ns;
    WriteLoan::write(loan)
}
