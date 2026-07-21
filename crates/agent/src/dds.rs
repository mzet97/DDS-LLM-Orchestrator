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
    /// Assina `Tasks`; para cada task elegível: claim otimista → readback de
    /// confirmação → processa em task tokio (se houver slot).
    pub async fn run<E: Engine + 'static>(self: Arc<Self>, engine: Arc<E>) -> Result<()> {
        let claim_cfg: ClaimConfig = self.agent.claim_config();
        let mut stream = Box::pin(self.dataspace.stream_tasks());

        tracing::info!(agent_id = %claim_cfg.agent_id, "claim loop iniciado");

        while let Some(task) = stream.next().await {
            if !claim::is_eligible(&task, &claim_cfg, &self.agent.claimed_set().await) {
                continue;
            }
            let task_id = task.task_id.clone();

            // Claim otimista: escreve ASSIGNED com o meu id
            let claimed_task = claim::claim_task(&task, &claim_cfg.agent_id);
            if let Err(e) = self.dataspace.write_task(claimed_task.clone()).await {
                tracing::warn!(task_id, error = %e, "claim: falha ao escrever ASSIGNED");
                continue;
            }

            // T-203: confirma ownership lendo o estado ARBITRADO do mesh (RHC),
            // após a janela de propagação — o cache local não decide arbitragem.
            tokio::time::sleep(CONFIRM_DELAY).await;
            let mine = match self.dataspace.read_task_mesh(&task_id) {
                Ok(Some(current)) => claim::confirm_ownership(&current, &claim_cfg.agent_id),
                _ => false,
            };
            if !mine {
                tracing::info!(task_id, "claim perdido na arbitragem (outro agente venceu)");
                continue;
            }

            self.agent.mark_claimed(task_id.clone()).await;

            let this = Arc::clone(&self);
            let engine = Arc::clone(&engine);
            tokio::spawn(async move {
                if let Err(e) = this.process_and_publish(&claimed_task, &*engine).await {
                    tracing::error!(task_id, error = %e, "processamento falhou");
                }
            });
        }

        Ok(())
    }

    /// Processa uma task claimed: RUNNING → inferência (chunks via pool) → DONE/FAILED.
    async fn process_and_publish<E: Engine>(&self, task: &Task, engine: &E) -> Result<()> {
        let task_id = task.task_id.clone();
        if !self.agent.status().acquire_slot() {
            anyhow::bail!("sem slots disponíveis");
        }

        // RUNNING
        let mut running = task.clone();
        running.status = 2;
        running.started_at_ns = now_ns();
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

        let latency_ms = start.elapsed().as_millis() as u64;
        let mut final_task = task.clone();
        final_task.completed_at_ns = now_ns();

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
