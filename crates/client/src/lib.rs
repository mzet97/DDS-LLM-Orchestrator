//! # Client — Submissão de tasks (Fase 3)
//!
//! Substitui `src/orchestrator/client/` (~0,2k LOC Python).
//! Resolve o deadlock de 20 clientes: UM participante servindo N tasks async.
//!
//! ## REQ-410/411
//! - `submit(task) -> Future<Result>` + stream de chunks
//! - ≥ 50 clientes concorrentes sem deadlock

use dds_contract::generated::dds_llm_orchestrator::Task;
use std::time::{SystemTime, UNIX_EPOCH};

/// Erro do cliente.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("timeout aguardando resultado da task {0}")]
    Timeout(String),
    #[error("task falhou: {0}")]
    TaskFailed(String),
    #[error("DDS error: {0}")]
    DdsError(String),
}

/// Configuração do cliente.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub client_id: String,
    pub dds_domain: u32,
    pub timeout_ms: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: format!("client-{}", uuid::Uuid::new_v4()),
            dds_domain: 0,
            timeout_ms: 120_000,
        }
    }
}

/// Resultado de uma task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub content: String,
    pub success: bool,
    pub latency_ms: u64,
    pub tokens_prompt: u32,
    pub tokens_completion: u32,
}

/// Cliente DDS — submete tasks e recebe resultados.
///
/// Usa UM participante DDS para N tasks async (resolve deadlock de 20).
/// Para DDS real, use `dds_impl::DdsClientDds` (feature-gated).
pub struct DdsClient {
    config: ClientConfig,
}

impl DdsClient {
    pub fn new(config: ClientConfig) -> Self {
        Self { config }
    }

    /// Cria uma task PENDING para submissão.
    pub fn create_task(
        &self,
        model: &str,
        messages_json: &str,
        priority: i32,
        stream: bool,
    ) -> Task {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Task {
            task_id: uuid::Uuid::new_v4().to_string(),
            client_id: self.config.client_id.clone(),
            assigned_agent: String::new(),
            target_agent: String::new(),
            model_required: 0,
            model_name: model.to_string(),
            messages_json: messages_json.to_string(),
            temperature: 0.7,
            max_tokens: 256,
            stream,
            status: 0, // PENDING
            priority,
            created_at_ns: now_ns,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns: now_ns + self.config.timeout_ms * 1_000_000,
            retry_count: 0,
            finish_reason: String::new(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        }
    }

    /// Submete task via HTTP (paridade com API do orchestrator).
    pub async fn submit_http(
        &self,
        orchestrator_url: &str,
        task: &Task,
    ) -> Result<String, ClientError> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/chat/completions", orchestrator_url);

        let messages: Vec<serde_json::Value> =
            serde_json::from_str(&task.messages_json).unwrap_or_default();

        let body = serde_json::json!({
            "model": task.model_name,
            "messages": messages,
            "temperature": task.temperature,
            "max_tokens": task.max_tokens,
            "stream": task.stream,
        });

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClientError::DdsError(e.to_string()))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ClientError::DdsError(e.to_string()))?;

        data.get("task_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ClientError::DdsError("resposta sem task_id".into()))
    }
}

// ── Implementação DDS real (T-410, REQ-410/411) ────────────────────────────

/// Cliente DDS real — UM participante servindo N tasks async (resolve o
/// deadlock de 20 clientes do Python: cada cliente Python criava um
/// DDSDataSpace com 17 tópicos+threads e o GIL travava em 20).
#[cfg(feature = "dds")]
pub mod dds_impl {
    use super::{ClientConfig, ClientError, TaskResult};
    use async_stream::stream;
    use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput};
    use dds_dataspace::api::DataSpaceApi;
    use dds_dataspace::DataSpace;
    use futures_core::Stream;
    use futures_util::StreamExt;
    use std::pin::Pin;
    use std::time::{Duration, Instant};

    /// Strength do papel cliente (Fase 2.2): 10 < agente(100) < orq(200).
    const STRENGTH_CLIENT: i32 = 10;

    pub struct DdsClientDds {
        config: ClientConfig,
        dataspace: DataSpace,
    }

    impl DdsClientDds {
        /// Cria o cliente com UM participante no domínio.
        pub fn new(config: ClientConfig) -> Result<Self, ClientError> {
            let dataspace = DataSpace::new(config.dds_domain, STRENGTH_CLIENT)
                .map_err(|e| ClientError::DdsError(e.to_string()))?;
            Ok(Self { config, dataspace })
        }

        pub fn dataspace(&self) -> &DataSpace {
            &self.dataspace
        }

        /// Submete a task e aguarda o resultado completo (DONE/FAILED).
        pub async fn submit(&self, task: Task) -> Result<TaskResult, ClientError> {
            let task_id = task.task_id.clone();
            let start = Instant::now();
            self.dataspace
                .write_task(task)
                .await
                .map_err(|e| ClientError::DdsError(e.to_string()))?;

            let timeout = Duration::from_millis(self.config.timeout_ms);
            let deadline = start + timeout;
            let mut status_stream = Box::pin(self.dataspace.stream_tasks());
            let mut outputs_stream = Box::pin(self.dataspace.stream_task_outputs());

            let mut chunks: Vec<TaskOutput> = Vec::new();
            // `status_stream` (Task DONE) e `outputs_stream` (TaskOutput chunks) são
            // tópicos DIFERENTES, lidos por readers independentes — não há garantia
            // de ordem de entrega ENTRE eles, mesmo que o agente escreva o último
            // chunk antes do DONE. Retornar assim que status==DONE chega é uma
            // condição de corrida real: o reader de status pode entregar antes do
            // reader de outputs ter recebido o(s) chunk(s) final(is), produzindo
            // `content` vazio ou truncado (reproduzido de forma determinística ao
            // rotear os writers de `Tasks` do agente por um pool — a mudança de
            // timing relativo entre os dois tópicos bastou para expor a corrida
            // pré-existente). Fix: só finaliza quando as DUAS condições forem
            // verdadeiras — status==DONE recebido E um chunk com `is_final` já
            // coletado — não importa em qual ordem os dois sinais chegam.
            let mut done = false;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(ClientError::Timeout(task_id));
                }
                tokio::select! {
                    out = outputs_stream.next() => {
                        if let Some(o) = out {
                            if o.task_id == task_id {
                                chunks.push((*o).clone());
                            }
                        }
                    }
                    st = status_stream.next() => {
                        if let Some(t) = st {
                            if t.task_id != task_id { continue; }
                            if t.status == 3 {
                                done = true;
                            } else if t.status == 4 {
                                // FAILED
                                return Err(ClientError::TaskFailed(t.finish_reason.clone()));
                            }
                        }
                    }
                }
                if done && chunks.iter().any(|c| c.is_final) {
                    chunks.sort_by_key(|c| c.seq_num);
                    let content: String = chunks.iter().map(|c| c.content.clone()).collect();
                    let tokens_completion = chunks.last().map(|c| c.token_count).unwrap_or(0);
                    return Ok(TaskResult {
                        task_id,
                        content,
                        success: true,
                        latency_ms: start.elapsed().as_millis() as u64,
                        tokens_prompt: 0,
                        tokens_completion,
                    });
                }
            }
        }

        /// Submete e retorna um stream de chunks (`TaskOutput`) até `is_final`.
        pub fn submit_stream(
            &self,
            task: Task,
        ) -> Pin<Box<dyn Stream<Item = Result<TaskOutput, ClientError>> + Send + '_>> {
            let task_id = task.task_id.clone();
            Box::pin(stream! {
                if let Err(e) = self.dataspace.write_task(task).await {
                    yield Err(ClientError::DdsError(e.to_string()));
                    return;
                }

                let timeout = Duration::from_millis(self.config.timeout_ms);
                let deadline = Instant::now() + timeout;
                let mut outputs = Box::pin(self.dataspace.stream_task_outputs());
                let mut status = Box::pin(self.dataspace.stream_tasks());

                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        yield Err(ClientError::Timeout(task_id));
                        return;
                    }
                    tokio::select! {
                        out = outputs.next() => {
                            match out {
                                Some(o) if o.task_id == task_id => {
                                    let is_final = o.is_final;
                                    yield Ok((*o).clone());
                                    if is_final { return; }
                                }
                                Some(_) => continue,
                                None => return,
                            }
                        }
                        st = status.next() => {
                            if let Some(t) = st {
                                if t.task_id == task_id && t.status == 4 {
                                    yield Err(ClientError::TaskFailed(t.finish_reason.clone()));
                                    return;
                                }
                            }
                        }
                        _ = tokio::time::sleep(remaining) => {
                            yield Err(ClientError::Timeout(task_id));
                            return;
                        }
                    }
                }
            })
        }
    }
}
