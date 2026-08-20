//! Runtime DDS do agente (T-202/T-203/T-204/T-206).
//!
//! - Claim loop: assina `Tasks`, filtra elegíveis, claim otimista (ASSIGNED),
//!   confirma ownership via readback após a janela de propagação.
//! - Processa tasks em tasks tokio (slots limitados por `AgentStatus`).
//! - Publica `TaskOutput` pelo pool MPMC do DataSpace.
//! - Heartbeat dedicado: `AgentState` a cada 5 s (Liveliness ManualByTopic, lease 10 s).

use crate::claim::{self, ClaimConfig};
use crate::engine::{Engine, InferRequest};
use crate::heartbeat::SlotGuard;
use crate::{Agent, AgentConfig};
use anyhow::Result;
use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::writer_pool::{WriteRequest, WriterPool};
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Janela de propagação para a confirmação de ownership (readback).
const CONFIRM_DELAY: Duration = Duration::from_millis(250);
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout do ack do write final de TaskOutput (RUST-PROTO-005): se o pool
/// não confirmar o `dds_write` do chunk final nesse prazo, a task vai para
/// FAILED com causa observável em vez de DONE sem saída final.
const FINAL_WRITE_ACK_TIMEOUT: Duration = Duration::from_secs(10);

/// Item enfileirado para claim (RUST-CLAIM-012): carrega o instante de
/// enfileiramento para medir tempo de fila e detectar task vencida enquanto
/// aguardava capacidade.
struct QueuedClaim {
    task: Arc<Task>,
    enqueued_at: Instant,
}

/// Runtime do agente sobre o DataSpace real.
pub struct AgentDds {
    agent: Agent,
    dataspace: Arc<DataSpace>,
    writer_pool: Arc<WriterPool>,
    claim_permits: Arc<Semaphore>,
}

impl AgentDds {
    /// Sobe o runtime: DataSpace com strength de agente + pool de writers.
    pub fn new(config: AgentConfig) -> Result<Self> {
        anyhow::ensure!(config.slots > 0, "agent precisa ter ao menos um slot");
        let dataspace = DataSpace::new(config.dds_domain, DataSpace::STRENGTH_AGENT)?;
        Self::build(config, dataspace)
    }

    #[cfg(feature = "security")]
    pub fn new_with_security(
        config: AgentConfig,
        security: Option<dds_dataspace::SecurityConfig>,
    ) -> Result<Self> {
        anyhow::ensure!(config.slots > 0, "agent precisa ter ao menos um slot");
        let dataspace = DataSpace::new_with_profile_and_security(
            config.dds_domain,
            DataSpace::STRENGTH_AGENT,
            None,
            security,
        )?;
        Self::build(config, dataspace)
    }

    fn build(config: AgentConfig, dataspace: DataSpace) -> Result<Self> {
        let writer_pool = dataspace.new_writer_pool(2, 4096);
        Ok(Self {
            claim_permits: Arc::new(Semaphore::new(config.slots as usize)),
            agent: Agent::new(config),
            dataspace: Arc::new(dataspace),
            writer_pool: Arc::new(writer_pool),
        })
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    pub fn dataspace(&self) -> &Arc<DataSpace> {
        &self.dataspace
    }

    /// T-206: heartbeat dedicado — publica `AgentState` a cada 5 s.
    /// Não congela durante inferência longa (task tokio própria).
    pub fn spawn_heartbeat(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let ds = Arc::clone(&self.dataspace);
        let status = self.agent.status();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = ds.write_agent_state(status.to_dds()).await {
                    tracing::warn!(error = %e, "heartbeat: falha ao publicar AgentState");
                }
            }
        })
    }

    /// T-202/T-203: claim loop principal.
    ///
    /// Duas camadas separadas por uma fila bounded (RUST-CLAIM-012):
    ///
    /// 1. **Ingestão** (task tokio dedicada): drena `stream_tasks()` sem
    ///    parar, aplica o pré-filtro barato de elegibilidade e enfileira em
    ///    um canal bounded (`4×slots`, mín. 16). O consumidor DDS nunca fica
    ///    preso atrás de um permit — no máximo atrás da fila cheia
    ///    (backpressure explícito, sem perda silenciosa).
    /// 2. **Dispatcher** (este loop): espera o permit do semáforo, REVALIDA
    ///    a task contra o cache (estado fresco — pode ter sido cancelada,
    ///    vencida pelo deadline ou claimed por outro agente enquanto
    ///    aguardava na fila) e só então dispara a tentativa de claim
    ///    (write ASSIGNED → `CONFIRM_DELAY` → readback → processa) como
    ///    task tokio independente. Várias janelas de confirmação correm em
    ///    paralelo (ver histórico no `OPTIMIZATION_REPORT.md`).
    pub async fn run<E: Engine + 'static>(self: Arc<Self>, engine: Arc<E>) -> Result<()> {
        let claim_cfg: ClaimConfig = self.agent.claim_config();
        let queue_cap = (self.agent.config.slots as usize * 4).max(16);
        let (tx, mut rx) = tokio::sync::mpsc::channel::<QueuedClaim>(queue_cap);

        tracing::info!(agent_id = %claim_cfg.agent_id, queue_cap, "claim loop iniciado");

        // RUST-CACHE-006: sweeper de eviction também no agente (antes só o
        // orquestrador tinha) — o cache local não cresce sem limite em
        // campanhas longas e o upsert sob pressão consegue aliviar terminais.
        let sweeper = {
            let ds = Arc::clone(&self.dataspace);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(15));
                loop {
                    interval.tick().await;
                    ds.caches().evict_terminal_tasks(Duration::from_secs(30));
                }
            })
        };

        // Ingestão: drena o stream DDS para a fila bounded.
        let ingestion = {
            let this = Arc::clone(&self);
            let cfg = claim_cfg.clone();
            tokio::spawn(async move {
                let mut stream = Box::pin(this.dataspace.stream_tasks());
                while let Some(task) = stream.next().await {
                    if !claim::is_eligible(&task, &cfg, &this.agent.claimed_set().await) {
                        continue;
                    }
                    let queued = QueuedClaim {
                        task,
                        enqueued_at: Instant::now(),
                    };
                    // Fila cheia → backpressure explícito (aguarda vaga); a
                    // task NÃO é descartada silenciosamente.
                    if tx.send(queued).await.is_err() {
                        break; // dispatcher encerrou
                    }
                }
            })
        };

        // Dispatcher: 1 permit por tentativa de claim, com revalidação
        // imediatamente antes do write de ASSIGNED.
        while let Some(queued) = rx.recv().await {
            let task_id = queued.task.task_id.clone();
            let permit = Arc::clone(&self.claim_permits)
                .acquire_owned()
                .await
                .expect("semaphore de admissão não é fechada");

            // Revalidação com estado FRESCO do cache (RUST-CLAIM-012): a
            // amostra que entrou na fila pode estar obsoleta.
            let fresh = match self.dataspace.caches().read_task(&task_id) {
                Some(t) => t,
                None => {
                    tracing::debug!(task_id, "claim descartado: task não está no cache");
                    continue;
                }
            };
            if !claim::is_eligible(&fresh, &claim_cfg, &self.agent.claimed_set().await) {
                continue;
            }
            if fresh.deadline_ns > 0 && fresh.deadline_ns <= now_ns() {
                tracing::info!(
                    task_id,
                    queue_wait_ms = queued.enqueued_at.elapsed().as_millis() as u64,
                    "claim descartado: task venceu aguardando capacidade"
                );
                continue;
            }
            tracing::debug!(
                task_id,
                queue_wait_ms = queued.enqueued_at.elapsed().as_millis() as u64,
                queue_len = rx.len(),
                "claim dispatch"
            );

            // Reserva JÁ, antes do write: impede que uma reentrega da mesma
            // task (ainda PENDING na visão local antes do nosso write
            // propagar de volta) dispare uma segunda tentativa concorrente
            // enquanto a primeira está na janela de confirmação.
            self.agent.mark_claimed(task_id).await;

            let this = Arc::clone(&self);
            let engine = Arc::clone(&engine);
            let claim_cfg = claim_cfg.clone();
            tokio::spawn(async move {
                this.attempt_claim_and_process(fresh, &claim_cfg, &*engine, permit)
                    .await;
            });
        }

        ingestion.abort();
        sweeper.abort();
        Ok(())
    }

    /// Uma tentativa de claim completa: claim otimista → readback de
    /// confirmação (T-203) → processa (se confirmado). Roda como task tokio
    /// independente, disparada por [`AgentDds::run`] — ver o comentário lá
    /// sobre por que isso não pode rodar inline no loop principal.
    async fn attempt_claim_and_process<E: Engine>(
        self: Arc<Self>,
        task: Arc<Task>,
        claim_cfg: &ClaimConfig,
        engine: &E,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) {
        let task_id = task.task_id.clone();

        // Claim otimista: escreve ASSIGNED com o meu id
        let claimed_task = claim::claim_task(&task, &claim_cfg.agent_id);
        if let Err(e) = self.dataspace.write_task(claimed_task.clone()).await {
            tracing::warn!(task_id, error = %e, "claim: falha ao escrever ASSIGNED");
            self.agent.unmark_claimed(&task_id).await;
            return;
        }

        // T-203: confirma ownership no RHC por handle de instância. Essa leitura
        // independe do stream de ingestão, que pode estar sob backpressure com
        // a fila cheia, e não varre o limite global de 256 amostras.
        let confirmation_deadline = Instant::now() + CONFIRM_TIMEOUT;
        let mine = loop {
            tokio::time::sleep(CONFIRM_DELAY).await;
            match self.dataspace.read_task_mesh(&task_id) {
                Err(e) => {
                    tracing::warn!(task_id, error = %e, "claim: falha ao confirmar ownership");
                    if Instant::now() >= confirmation_deadline {
                        break false;
                    }
                }
                Ok(Some(current)) if claim::confirm_ownership(&current, &claim_cfg.agent_id) => {
                    break true;
                }
                Ok(Some(current)) if current.status != 0 => break false,
                Ok(_) if Instant::now() >= confirmation_deadline => break false,
                Ok(_) => {}
            }
        };
        if !mine {
            tracing::info!(task_id, "claim perdido na arbitragem (outro agente venceu)");
            self.agent.unmark_claimed(&task_id).await;
            return;
        }

        // A partir daqui a task é NOSSA (ASSIGNED, confirmada). O guard RAII
        // amarra o permit do semáforo e o `slots_busy` numa única posse
        // (RUST-SLOT-007): sucesso, erro `?`, panic contido, cancelamento e
        // shutdown liberam ambos via Drop. O `assert!` anterior panicava o
        // agente quando as duas contabilidades divergiam (após uma saída `?`
        // que pulava o decremento); agora a divergência é erro operacional
        // tipado e a task é abortada sem derrubar o processo.
        let _slot = match SlotGuard::acquire(Some(permit), self.agent.status()) {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!(
                    task_id,
                    error = %e,
                    "capacidade divergente; task abortada sem panic"
                );
                self.agent.unmark_claimed(&task_id).await;
                return;
            }
        };

        if let Err(e) = self.process_and_publish(&claimed_task, engine).await {
            tracing::error!(task_id, error = %e, "processamento falhou");
        }
        self.agent.unmark_claimed(&task_id).await;
    }

    /// Processa uma task claimed: RUNNING → inferência (chunks via pool) → DONE/FAILED.
    ///
    /// Pré-condição: o chamador ([`AgentDds::attempt_claim_and_process`])
    /// já reservou a capacidade via [`SlotGuard`] (permit + `slots_busy`);
    /// este método não toca na contabilidade de slots — o Drop do guard
    /// libera em qualquer saída, inclusive nos `?` abaixo.
    ///
    /// Terminalidade (RUST-PROTO-005): DONE só é publicado depois do ack do
    /// `dds_write` do chunk final (submit_with_ack + timeout). Falha ou
    /// timeout do write final vira FAILED com causa observável — enqueue
    /// sozinho nunca conta como entrega.
    async fn process_and_publish<E: Engine>(&self, task: &Task, engine: &E) -> Result<()> {
        let task_id = task.task_id.clone();

        // RUNNING
        let mut running = task.clone();
        running.status = 2;
        running.started_at_ns = now_ns();
        // T-E1: tempo de fila no agente (claim confirmado → início da
        // inferência), inteiramente local a este processo — sem risco de
        // desvio de relógio entre máquinas (ver OPTIMIZATION_REPORT.md,
        // "Itens pendentes" — os campos t_*_ns nunca eram populados até
        // aqui). Os componentes de transporte/serialização (t_transport_*,
        // t_*serialization_ns) cruzam processos e ficam de fora
        // deliberadamente: medi-los exigiria comparar relógios de máquinas
        // distintas, o que a própria dissertação evita (§protocolo de
        // medição) calculando-os por diferença a partir do T_total
        // observado no cliente, não por timestamp direto aqui.
        let t_agent_queue_ns = running.started_at_ns.saturating_sub(task.assigned_at_ns);
        self.dataspace.write_task(running).await?;

        let timeout_ms = task
            .deadline_ns
            .saturating_sub(now_ns())
            .saturating_div(1_000_000)
            .max(1_000);

        let req = InferRequest {
            request_id: task_id.clone(),
            messages_json: task.messages_json.clone(),
            model_name: task.model_name.clone(),
            temperature: task.temperature,
            max_tokens: task.max_tokens,
            stream: task.stream,
            timeout_ms,
        };

        let start = Instant::now();
        let mut stream = engine.infer_stream(req);
        let mut failed: Option<String> = None;
        let mut final_ack = None;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let out = TaskOutput {
                        task_id: task_id.clone(),
                        seq_num: chunk.seq_num,
                        content: chunk.content,
                        is_final: chunk.is_final,
                        finish_reason: if chunk.is_final { 1 } else { 0 },
                        agent_id: self.agent.config.agent_id.clone(),
                        token_count: chunk.tokens_completion,
                        emitted_at_ns: now_ns(),
                    };
                    if out.is_final {
                        // Chunk final: exige ack do dds_write (RUST-PROTO-005).
                        match self.writer_pool.submit_with_ack(out) {
                            Ok(rx) => final_ack = Some(rx),
                            Err(e) => {
                                let error = format!("falha ao enfileirar output final DDS: {e}");
                                tracing::warn!(task_id, error = %error, "backpressure no pool de outputs");
                                failed = Some(error);
                                break;
                            }
                        }
                    } else if let Err(e) = self.writer_pool.submit(WriteRequest::Output(out)) {
                        let error = format!("falha ao enfileirar output DDS: {e}");
                        tracing::warn!(task_id, error = %error, "backpressure no pool de outputs");
                        failed = Some(error);
                        break;
                    }
                }
                Err(e) => {
                    failed = Some(e.to_string());
                    break;
                }
            }
        }

        let inference_elapsed = start.elapsed();
        let latency_ms = inference_elapsed.as_millis() as u64;
        let mut final_task = task.clone();
        final_task.completed_at_ns = now_ns();
        // T-E1 (continuação): tempo de inferência, também local — a mesma
        // janela que já produzia `latency_ms`, só que em ns e persistida na
        // task em vez de só logada.
        final_task.t_agent_queue_ns = t_agent_queue_ns;
        final_task.t_inference_ns = inference_elapsed.as_nanos() as u64;

        if failed.is_none() {
            // RUST-PROTO-005: DONE exige o ack do dds_write do chunk final.
            failed = match final_ack {
                Some(rx) => match tokio::time::timeout(FINAL_WRITE_ACK_TIMEOUT, rx).await {
                    Ok(Ok(Ok(()))) => None, // write final confirmado pelo pool
                    Ok(Ok(Err(e))) => Some(format!("write final DDS falhou: {e}")),
                    Ok(Err(_closed)) => {
                        Some("writer pool encerrou sem confirmar o write final".into())
                    }
                    Err(_elapsed) => Some(format!(
                        "timeout ({FINAL_WRITE_ACK_TIMEOUT:?}) aguardando ack do write final"
                    )),
                },
                None => Some("inferência terminou sem output final publicado".into()),
            };
        }

        match failed {
            None => {
                final_task.status = 3; // DONE
                final_task.finish_reason = "completion".into();
                self.dataspace.write_task(final_task).await?;
                self.agent.status().record_completion(latency_ms);
                tracing::info!(task_id, latency_ms, "task concluída");
            }
            Some(err_msg) => {
                final_task.status = 4; // FAILED
                final_task.finish_reason = err_msg.clone();
                self.dataspace.write_task(final_task).await?;
                self.agent.status().record_failure();
                anyhow::bail!(err_msg);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim::Specialization;

    #[test]
    fn rejects_zero_slots_before_opening_dds() {
        let result = AgentDds::new(AgentConfig {
            agent_id: "agent".into(),
            hostname: "host".into(),
            model: "model".into(),
            specialization: Specialization::Text,
            slots: 0,
            dds_domain: 0,
        });

        assert!(result.is_err());
    }
}
