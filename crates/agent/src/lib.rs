//! # Agent — 1º alvo da migração (maior ROI)
//!
//! Substitui `src/orchestrator/agent/` (~2,0k LOC Python): assume tasks PENDING
//! via DDS (claim com confirmação de ownership), faz a ponte com o llama-server
//! C++ e faz streaming dos chunks de volta.
//!
//! ## Módulos
//! - `engine`: Trait Engine + MockEngine + DdsEngine
//! - `claim`: Claim loop, seleção, confirmação de ownership
//! - `heartbeat`: Heartbeat dedicado (AgentState a cada 5s)

pub mod claim;
pub mod engine;
pub mod engine_http;
pub mod heartbeat;

#[cfg(feature = "dds")]
pub mod dds;
#[cfg(feature = "dds")]
pub mod engine_dds;

use anyhow::Result;
use claim::{ClaimConfig, Specialization};
use engine::{Engine, InferRequest};
use futures_util::StreamExt;
use heartbeat::{AgentStatus, SlotGuard};
use std::collections::HashSet;
use std::sync::Arc;

/// Configuração do agente.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub agent_id: String,
    pub hostname: String,
    pub model: String,
    pub specialization: Specialization,
    pub slots: u32,
    pub dds_domain: u32,
    /// Restrição de claim: só aceita tasks cujo `target_agent` comece com este
    /// prefixo. Vazio (padrão) = aceita qualquer target; tasks sem target
    /// (`target_agent` vazio) são sempre elegíveis (claim aberto).
    pub target_agent_prefix: String,
}

/// Agente principal — orquestra claim, inferência e heartbeat.
pub struct Agent {
    pub config: AgentConfig,
    status: Arc<AgentStatus>,
    /// T-820-09: `std::sync::RwLock` (não tokio) — as seções críticas são
    /// `insert`/`remove`/`contains` puros, sem `.await` sob lock, e o caminho
    /// quente de ingestão precisa de um point-check (`is_claimed`) sem clonar
    /// o conjunto por amostra DDS.
    claimed: Arc<std::sync::RwLock<HashSet<String>>>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        let status = Arc::new(AgentStatus::new(
            config.agent_id.clone(),
            config.hostname.clone(),
            config.model.clone(),
            format!("{:?}", config.specialization),
            config.slots,
        ));

        Self {
            config,
            status,
            claimed: Arc::new(std::sync::RwLock::new(HashSet::new())),
        }
    }

    /// Retorna a configuração de claim.
    pub fn claim_config(&self) -> ClaimConfig {
        ClaimConfig {
            agent_id: self.config.agent_id.clone(),
            specialization: self.config.specialization,
            target_agent_prefix: self.config.target_agent_prefix.clone(),
        }
    }

    /// Retorna o status compartilhado (para heartbeat).
    pub fn status(&self) -> Arc<AgentStatus> {
        self.status.clone()
    }

    /// Snapshot do conjunto de tasks já claimed (diagnóstico/compat).
    ///
    /// T-820-09: o caminho quente de elegibilidade NÃO deve usar este snapshot
    /// (clona o conjunto inteiro por amostra DDS) — use [`Agent::is_claimed`]
    /// via [`claim::is_eligible_with`](crate::claim::is_eligible_with).
    pub fn claimed_set(&self) -> HashSet<String> {
        self.claimed_read().clone()
    }

    /// Point-check O(1) "esta task já está reservada por este agente?"
    /// (T-820-09 — usado no filtro de elegibilidade do claim loop).
    pub fn is_claimed(&self, task_id: &str) -> bool {
        self.claimed_read().contains(task_id)
    }

    /// Marca uma task como claimed por este agente.
    ///
    /// Chamado no momento em que a tentativa de claim é disparada (antes do
    /// write de ASSIGNED), não só após a confirmação — isso é o que impede
    /// uma reentrega da mesma task no stream de disparar uma segunda
    /// tentativa concorrente enquanto a primeira ainda está na janela de
    /// confirmação (`CONFIRM_DELAY`). Se a tentativa falhar ou perder a
    /// arbitragem, o chamador deve desfazer com [`Agent::unmark_claimed`].
    pub fn mark_claimed(&self, task_id: String) {
        self.claimed_write().insert(task_id);
    }

    /// Desfaz uma reserva de [`Agent::mark_claimed`] quando a tentativa de
    /// claim falha (erro de escrita) ou perde a arbitragem (outro agente
    /// venceu) — sem isso a task ficaria presa como "claimed" para sempre
    /// neste agente e nunca mais seria elegível de novo.
    pub fn unmark_claimed(&self, task_id: &str) {
        self.claimed_write().remove(task_id);
    }

    /// Lock de leitura com recuperação de poisoning (mesma política do
    /// `RateLimiter` do llm-gateway): um panic alheio enquanto o lock estava
    /// tomado não pode derrubar o claim loop do agente.
    fn claimed_read(&self) -> std::sync::RwLockReadGuard<'_, HashSet<String>> {
        match self.claimed.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Lock de escrita com recuperação de poisoning (ver [`Agent::claimed_read`]).
    fn claimed_write(&self) -> std::sync::RwLockWriteGuard<'_, HashSet<String>> {
        match self.claimed.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Processa uma task claimed.
    pub async fn process_task<E: Engine>(
        &self,
        task: &dds_contract::generated::dds_llm_orchestrator::Task,
        engine: &E,
    ) -> Result<()> {
        // Reservar slot (guard RAII: qualquer saída libera slots_busy — RUST-SLOT-007)
        let _slot =
            SlotGuard::acquire(None, Arc::clone(&self.status)).map_err(|e| anyhow::anyhow!(e))?;

        let start = std::time::Instant::now();

        // Deriva timeout do deadline da task (margem de 5s antes do deadline)
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let timeout_ms = if task.deadline_ns > now_ns {
            ((task.deadline_ns - now_ns) / 1_000_000)
                .saturating_sub(5_000)
                .max(10_000)
        } else {
            120_000 // fallback: 120s
        };

        // Criar requisição de inferência
        let req = InferRequest {
            request_id: task.task_id.clone(),
            messages_json: task.messages_json.clone(),
            model_name: task.model_name.clone(),
            temperature: task.temperature,
            max_tokens: task.max_tokens,
            stream: task.stream,
            timeout_ms,
        };

        // Executar inferência e publicar chunks
        let mut stream = engine.infer_stream(req);
        let mut seq_num: u32 = 0;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // Nota: em modo DDS, use AgentDds::process_and_publish() em dds.rs
                    // que publica TaskOutput via WriterPool. Este path é o fallback
                    // não-DDS (mock/testes) — apenas loga os chunks.
                    tracing::debug!(
                        task_id = %task.task_id,
                        seq_num = chunk.seq_num,
                        content = %chunk.content,
                        "chunk processado (use AgentDds para publicar via DDS)"
                    );
                    seq_num = chunk.seq_num;

                    if chunk.is_final {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!(task_id = %task.task_id, error = %e, "erro na inferência");
                    self.status.record_failure();
                    return Err(e.into());
                }
            }
        }

        let latency = start.elapsed().as_millis() as u64;
        self.status.record_completion(latency);

        tracing::info!(
            task_id = %task.task_id,
            latency_ms = latency,
            chunks = seq_num + 1,
            "task concluída"
        );

        Ok(())
    }
}
