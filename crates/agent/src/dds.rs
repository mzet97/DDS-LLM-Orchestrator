//! Runtime DDS do agente (T-202/T-203/T-204/T-206).
//!
//! - Claim loop: assina `Tasks`, filtra elegíveis, claim otimista (ASSIGNED),
//!   confirma ownership via readback após a janela de propagação.
//! - Processa tasks em tasks tokio (slots limitados por `AgentStatus`).
//! - Publica `TaskOutput` pelo pool MPMC do DataSpace.
//! - Heartbeat dedicado: `AgentState` a cada 5 s (Liveliness ManualByTopic, lease 10 s).

use crate::claim::{self, ClaimConfig};
use crate::engine::{Engine, InferRequest};
use crate::{Agent, AgentConfig};
use anyhow::Result;
use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::writer_pool::{WriteRequest, WriterPool};
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Janela de propagação para a confirmação de ownership (readback).
const CONFIRM_DELAY: Duration = Duration::from_millis(250);

/// Intervalo de poll enquanto espera um slot de processamento livre (task já
/// claimed, mas todos os slots ocupados). Curto o bastante para não atrasar
/// perceptivelmente o início do processamento assim que um slot libera.
const SLOT_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Runtime do agente sobre o DataSpace real.
pub struct AgentDds {
    agent: Agent,
    dataspace: Arc<DataSpace>,
    writer_pool: Arc<WriterPool>,
}

impl AgentDds {
    /// Sobe o runtime: DataSpace com strength de agente + pool de writers.
    pub fn new(config: AgentConfig) -> Result<Self> {
        let dataspace = DataSpace::new(config.dds_domain, DataSpace::STRENGTH_AGENT)?;
        let writer_pool = dataspace.new_writer_pool(2, 4096);
        Ok(Self {
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
    /// Assina `Tasks` e, para cada task elegível, DISPARA a tentativa de
    /// claim (write ASSIGNED → espera `CONFIRM_DELAY` → readback de
    /// confirmação → processa) como sua própria task tokio, em vez de
    /// executá-la inline. O loop em si só faz trabalho em memória
    /// (`is_eligible`, `mark_claimed`) entre um `stream.next()` e outro —
    /// nunca bloqueia em I/O ou em `CONFIRM_DELAY`.
    ///
    /// Antes, os três passos da tentativa rodavam inline aqui, serializando
    /// todo o loop atrás do `sleep(CONFIRM_DELAY)` de 250 ms: nenhuma outra
    /// task podia sequer ser considerada enquanto essa espera não
    /// terminasse, travando o throughput de claims em ~1/CONFIRM_DELAY
    /// (~4/s) não importa quantas tasks elegíveis existissem ou quantos
    /// slots/GPU o agente tivesse disponíveis — medido empiricamente como o
    /// teto real do sistema (ver `OPTIMIZATION_REPORT.md`, achado do teste
    /// de carga). Agora as janelas de confirmação de várias tasks correm em
    /// paralelo entre si.
    pub async fn run<E: Engine + 'static>(self: Arc<Self>, engine: Arc<E>) -> Result<()> {
        let claim_cfg: ClaimConfig = self.agent.claim_config();
        let mut stream = Box::pin(self.dataspace.stream_tasks());

        tracing::info!(agent_id = %claim_cfg.agent_id, "claim loop iniciado");

        while let Some(task) = stream.next().await {
            if !claim::is_eligible(&task, &claim_cfg, &self.agent.claimed_set().await) {
                continue;
            }
            let task_id = task.task_id.clone();

            // Reserva JÁ, antes do write: impede que uma reentrega da mesma
            // task (ainda PENDING na visão local antes do nosso write
            // propagar de volta) dispare uma segunda tentativa concorrente
            // enquanto a primeira está na janela de confirmação.
            self.agent.mark_claimed(task_id.clone()).await;

            let this = Arc::clone(&self);
            let engine = Arc::clone(&engine);
            let claim_cfg = claim_cfg.clone();
            tokio::spawn(async move {
                this.attempt_claim_and_process(task, &claim_cfg, &*engine)
                    .await;
            });
        }

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
    ) {
        let task_id = task.task_id.clone();

        // Claim otimista: escreve ASSIGNED com o meu id
        let claimed_task = claim::claim_task(&task, &claim_cfg.agent_id);
        if let Err(e) = self.dataspace.write_task(claimed_task.clone()).await {
            tracing::warn!(task_id, error = %e, "claim: falha ao escrever ASSIGNED");
            self.agent.unmark_claimed(&task_id).await;
            return;
        }

        // T-203: confirma ownership lendo o estado ARBITRADO após a janela de
        // propagação. Usa `caches().read_task()` (upsert monotônico
        // alimentado pelo próprio consumo de `stream_tasks()` deste loop de
        // claim — Fase 5), NÃO `read_task_mesh()` (dds_read direto): esse
        // último faz um scan linear de até 256 amostras *somadas entre TODAS
        // as instâncias/tasks* no RHC do reader (`DataReader::read_impl`,
        // `max_samples = 256` fixo), e como o RHC nunca purga tasks
        // concluídas, isso satura em ~256/(amostras por task) tasks
        // processadas (medido: parede real em ~65 tasks, com ~4 amostras de
        // status por task) — depois disso a confirmação simplesmente não
        // encontra mais a própria task no scan e todo claim subsequente
        // "perde" a arbitragem, mesmo sendo o único agente. O cache não sofre
        // esse limite (é um DashMap por task_id, sem cap de leitura) e ainda
        // reflete o estado ARBITRADO de verdade: ownership Exclusive é
        // resolvido pelo próprio DDS antes da amostra sequer chegar a
        // qualquer reader — amostras perdedoras nunca aparecem em
        // `stream_tasks()`, então o que está no cache já é o vencedor.
        tokio::time::sleep(CONFIRM_DELAY).await;
        let mine = self
            .dataspace
            .caches()
            .read_task(&task_id)
            .is_some_and(|current| claim::confirm_ownership(&current, &claim_cfg.agent_id));
        if !mine {
            tracing::info!(task_id, "claim perdido na arbitragem (outro agente venceu)");
            self.agent.unmark_claimed(&task_id).await;
            return;
        }

        // A partir daqui a task é NOSSA (ASSIGNED, confirmada) mesmo que
        // ainda não haja slot livre para processá-la agora — ela fica só
        // "esperando vez" nesta task tokio, sem bloquear o loop principal
        // nem outras tentativas de claim. Isto é o que faz o gate de
        // capacidade (`process_and_publish`) nunca mais deixar uma task
        // presa em ASSIGNED para sempre: antes, achar "sem slot" ali
        // resultava em bail! definitivo (a task nunca mais era revisitada,
        // pois o mesh já não a reentrega depois do claim).
        while !self.agent.status().acquire_slot() {
            tokio::time::sleep(SLOT_POLL_INTERVAL).await;
        }

        if let Err(e) = self.process_and_publish(&claimed_task, engine).await {
            tracing::error!(task_id, error = %e, "processamento falhou");
        }
    }

    /// Processa uma task claimed: RUNNING → inferência (chunks via pool) → DONE/FAILED.
    ///
    /// Pré-condição: o chamador ([`AgentDds::attempt_claim_and_process`])
    /// já reservou o slot (espera até haver um livre); este método só o
    /// libera (via `record_completion`/`record_failure`), nunca reserva.
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
                    if let Err(e) = self.writer_pool.submit(WriteRequest::Output(out)) {
                        tracing::warn!(task_id, error = %e, "backpressure no pool de outputs");
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
