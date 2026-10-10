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
use tokio::sync::oneshot;

/// Pedido de escrita genérico.
pub enum WriteRequest {
    Task(Task),
    Agent(AgentState),
    Output(TaskOutput),
    /// Chunk final de um stream: o worker confirma o resultado real do
    /// `dds_write` pelo canal (RUST-PROTO-005 — o agente só publica DONE
    /// depois desse ack; falha/timeout vira FAILED com causa observável).
    OutputAck(TaskOutput, oneshot::Sender<Result<(), DataSpaceError>>),
}

/// Closure de escrita: devolve o resultado REAL do `dds_write` para que o
/// pool conte `completed`/`failed` corretamente (T-820-06 — antes o
/// `completed` incrementava mesmo com write falho e a falha era só log).
pub type WriteFn = Arc<dyn Fn(WriteRequest) -> Result<(), DataSpaceError> + Send + Sync>;

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
    /// `write_fn` recebe o pedido, escreve no DDS e devolve o resultado.
    /// Falível: spawn de thread pode falhar (exaustão de recursos) — `Err`
    /// em vez de panic na inicialização.
    pub fn new(
        n_workers: usize,
        capacity: usize,
        write_fn: WriteFn,
    ) -> Result<Self, DataSpaceError> {
        let (tx, rx) = crossbeam_channel::bounded(capacity);
        let submitted = Arc::new(AtomicU64::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));

        let mut workers = Vec::with_capacity(n_workers);
        for i in 0..n_workers {
            let rx: Receiver<WriteRequest> = rx.clone();
            let write_fn = Arc::clone(&write_fn);
            let completed = Arc::clone(&completed);
            let failed = Arc::clone(&failed);
            workers.push(
                std::thread::Builder::new()
                    .name(format!("dds-writer-{i}"))
                    .spawn(move || {
                        while let Ok(req) = rx.recv() {
                            // T-820-06: `completed` só conta `dds_write`
                            // bem-sucedido; falha de write conta em `failed`
                            // (além das falhas de enqueue já contadas em
                            // `submit`).
                            match write_fn(req) {
                                Ok(()) => {
                                    completed.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "writer_pool: write falhou (dds_write)");
                                    failed.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        // canal fechado + drenado → sai
                    })
                    .map_err(|e| {
                        DataSpaceError::WriteFailed(format!("spawn dds-writer-{i}: {e}"))
                    })?,
            );
        }

        Ok(Self {
            tx,
            workers,
            submitted,
            completed,
            failed,
        })
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

    /// Enfileira o write FINAL de um stream e devolve o canal de confirmação
    /// (RUST-PROTO-005). O ack carrega o resultado real do `dds_write` feito
    /// pelo worker — enqueue com sucesso NÃO conta como entrega.
    ///
    /// Shutdown: `drain_and_shutdown` drena o canal antes de encerrar os
    /// workers, então todo ack enfileirado é respondido; se o pool for
    /// dropado sem drain, o receiver observa canal fechado (falha explícita).
    pub fn submit_with_ack(
        &self,
        output: TaskOutput,
    ) -> Result<oneshot::Receiver<Result<(), DataSpaceError>>, DataSpaceError> {
        let (tx, rx) = oneshot::channel();
        self.submit(WriteRequest::OutputAck(output, tx))?;
        Ok(rx)
    }

    pub fn submitted(&self) -> u64 {
        self.submitted.load(Ordering::Relaxed)
    }
    /// Escritas concluídas com `dds_write` bem-sucedido (T-820-06: antes
    /// contava também as falhas de write — métrica enganosa).
    pub fn completed(&self) -> u64 {
        self.completed.load(Ordering::Relaxed)
    }
    /// Falhas: enqueue rejeitado (backpressure/pool encerrado) + `dds_write`
    /// que retornou erro (T-820-06).
    pub fn failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    /// Fecha o canal e espera os workers drenarem, devolvendo as estatísticas
    /// finais `(submitted, completed, failed)` — só são estáveis após o join
    /// dos workers (T-820-06: ler antes do drain é corrida).
    pub fn drain_and_shutdown(self) -> (u64, u64, u64) {
        let WriterPool {
            tx,
            workers,
            submitted,
            completed,
            failed,
        } = self;
        drop(tx);
        for (i, w) in workers.into_iter().enumerate() {
            if let Err(e) = w.join() {
                tracing::error!(worker = i, error = ?e, "writer_pool: worker panou");
            }
        }
        (
            submitted.load(Ordering::Relaxed),
            completed.load(Ordering::Relaxed),
            failed.load(Ordering::Relaxed),
        )
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
///
/// Falível (T-820-06): `tasks_writers` vazio era panic por indexação
/// (`tasks_writers[idx]`) no primeiro `WriteRequest::Task` — agora devolve
/// `Err` na construção. Os logs de falha de write continuam aqui (contexto do
/// request); o resultado é devolvido para o pool contabilizar `failed`.
pub fn make_write_fn(
    tasks_writers: Vec<DataWriter<Task>>,
    agents_writer: DataWriter<AgentState>,
    outputs_writer: DataWriter<TaskOutput>,
) -> Result<WriteFn, DataSpaceError> {
    if tasks_writers.is_empty() {
        return Err(DataSpaceError::WriteFailed(
            "make_write_fn: pool de writers de Tasks vazio".into(),
        ));
    }
    Ok(Arc::new(move |req| {
        // Match único por valor: cada variante é tratada no próprio braço,
        // sem pré-checagem + `unreachable!`.
        match req {
            // Variante com confirmação: o resultado REAL do dds_write vai para o
            // canal de ack (RUST-PROTO-005). Se o receiver já desistiu (timeout/
            // cancelamento), o send falha sem custo — o erro continua logado.
            WriteRequest::OutputAck(o, ack) => match write_output_loan(&outputs_writer, &o) {
                Ok(()) => {
                    let _ = ack.send(Ok(()));
                    Ok(())
                }
                Err(e) => {
                    let msg = e.to_string();
                    let err = DataSpaceError::WriteFailed(msg.clone());
                    tracing::error!(error = %err, "writer_pool: falha no write FINAL do DDS");
                    let _ = ack.send(Err(DataSpaceError::WriteFailed(msg)));
                    Err(err)
                }
            },
            WriteRequest::Task(t) => {
                let idx = crate::select_task_writer_slot(&t.task_id, tasks_writers.len());
                match tasks_writers[idx].write(&t) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        tracing::error!(error = %e, "writer_pool: falha ao escrever no DDS");
                        Err(DataSpaceError::WriteFailed(e.to_string()))
                    }
                }
            }
            WriteRequest::Agent(a) => match agents_writer.write(&a) {
                Ok(()) => Ok(()),
                Err(e) => {
                    tracing::error!(error = %e, "writer_pool: falha ao escrever no DDS");
                    Err(DataSpaceError::WriteFailed(e.to_string()))
                }
            },
            // Zero-copy: TaskOutput é o tópico de maior volume de samples (um
            // por chunk de streaming de inferência) — T-616. Ver
            // `write_output_loan` para o porquê do loan em vez de `.write()`.
            WriteRequest::Output(o) => match write_output_loan(&outputs_writer, &o) {
                Ok(()) => Ok(()),
                Err(e) => {
                    tracing::error!(error = %e, "writer_pool: falha ao escrever no DDS");
                    Err(DataSpaceError::WriteFailed(e.to_string()))
                }
            },
        }
    }))
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
    // SAFETY (`WriteLoan::get_mut` exige preservar os invariantes de `Native`):
    // (1) o buffer foi alocado e zerado pelo CycloneDDS com exatamente
    // `size_of::<Native>()` bytes, e o estado zerado é válido (ponteiro nulo
    // para `DdsString`, zero válido para primitivos) — ver `request_loan`;
    // (2) abaixo TODOS os 8 campos são atribuídos: escalares por cópia e
    // strings via `DdsString::new` (construtor checado — NUL vira `Err`);
    // (3) `Native` não tem enums (discriminantes) nem opcionais com
    // proveniência — nada além do atribuído precisa ser preservado.
    // Miri é inviável aqui (exige participant DDS vivo + FFI C).
    let native = unsafe { loan.get_mut() };
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
