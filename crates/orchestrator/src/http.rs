//! Fronteira HTTP do orquestrador.
//!
//! # T-850-04 — instrumentação T1–T6 (mapeamento da dissertação)
//!
//! | Medida | Campo | Dono | Onde é preenchida nesta fronteira |
//! |---|---|---|---|
//! | T1 serialização | `t_serialization_ns` | orquestrador (HTTP) | medido em volta da construção/validação do `Task` (inclui `messages_json`) e gravado **no Task publicado** (viaja pelo DDS); também ecoado na resposta HTTP |
//! | T2 transporte (ida) | `t_transport_send_ns` | orquestrador (HTTP) | duração do `publish_task` (o write DDS; a chamada retorna após o `dds_write`). Só é conhecido **depois** do write — a cópia que trafega fica 0 (não fabricar zero), o valor medido vai na resposta HTTP |
//! | T3 fila do agente | `t_agent_queue_ns` | agente | preenchido pelo agente no write terminal (`agent/src/dds.rs`); o `/sync` repassa o valor do task terminal |
//! | T4 inferência | `t_inference_ns` | agente | idem T3 |
//! | T5 transporte (volta) | `t_transport_return_ns` | cliente | no `/sync` o handler **é** o cliente: (instante da leitura terminal, relógio de parede) − `completed_at_ns` do task terminal = janela entre o agente publicar o estado terminal e o poll observá-lo (inclui a granularidade de 25 ms do poll); skew de relógio → `saturating_sub` → 0 |
//! | T6 desserialização | `t_deserialization_ns` | cliente | no `/sync`: duração da leitura terminal (`read_task` no cache + clone) na iteração que fechou o estado |
//!
//! No handler assíncrono só T1/T2 existem (a resposta volta antes da
//! execução); T3–T6 de tasks publicados assincronamente são coletados pelo
//! driver de benchmarks lendo o `TaskOutput`/task terminal.

use crate::http_config::{CallerIdentity, HttpConfig};
use async_trait::async_trait;
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Extension, Request, State},
    http::{header::WWW_AUTHENTICATE, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{sync::Arc, time::Duration, time::Instant};
use tokio::sync::Semaphore;

#[derive(Debug, thiserror::Error)]
#[error("DDS operation failed")]
pub struct HttpBackendError;

#[async_trait]
pub trait HttpBackend: Send + Sync {
    async fn publish_task(&self, task: Task) -> Result<(), HttpBackendError>;
    /// EXP1b (dissertação §3.7.4): em modo despacho, o orquestrador escolhe o
    /// agente (menor `slots_busy` entre os disponíveis) e fixa `target_agent`,
    /// mantendo a task PENDING — o claim+readback do agente permanece
    /// intacto; a variável isolada é QUEM DECIDE a atribuição.
    /// Default: sem despacho (claim distribuído, caminho principal).
    async fn dispatch_task(&self, _task: &mut Task) -> bool {
        false
    }
    fn read_task(&self, task_id: &str) -> Option<Task>;
    fn agents(&self) -> Vec<AgentState>;
}

#[derive(Clone)]
struct HttpState {
    backend: Arc<dyn HttpBackend>,
    config: Arc<HttpConfig>,
    concurrency: Arc<Semaphore>,
}

#[derive(Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    stream: Option<bool>,
}

#[derive(Deserialize, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatResponse {
    task_id: String,
    status: &'static str,
    /// T-850-04/T1: serialização do Task (também vai no Task publicado).
    t_serialization_ns: u64,
    /// T-850-04/T2: duração do publish (DDS write). No Task que trafega o
    /// campo é 0 por construção (o write termina depois que a cópia sai);
    /// o valor medido é reportado aqui.
    t_transport_send_ns: u64,
}

#[derive(Debug)]
enum HttpError {
    Unauthorized,
    Forbidden,
    PayloadTooLarge,
    Unprocessable,
    TooManyRequests,
    GatewayTimeout,
    Internal,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            Self::Unprocessable => (StatusCode::UNPROCESSABLE_ENTITY, "unprocessable_request"),
            Self::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "request_quota_exceeded"),
            Self::GatewayTimeout => (StatusCode::GATEWAY_TIMEOUT, "dds_wait_timeout"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        let mut response = (status, Json(json!({ "error": code }))).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

#[doc = "Builds the authenticated and resource-bounded HTTP router (REQ-704)."]
pub fn router(config: HttpConfig, backend: Arc<dyn HttpBackend>) -> Router {
    let body_limit = config.limits.body_bytes;
    let state = HttpState {
        concurrency: Arc::new(Semaphore::new(config.limits.concurrent_requests)),
        config: Arc::new(config),
        backend,
    };
    let protected = Router::new()
        .route("/api/v1/chat/completions", post(submit_task))
        .route("/api/v1/chat/completions/sync", post(submit_task_sync))
        .route("/api/v1/agents", get(list_agents))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authorize_and_limit,
        ));

    Router::new()
        .route("/health", get(health))
        .merge(protected)
        .layer(DefaultBodyLimit::max(body_limit))
        .with_state(state)
}

async fn authorize_and_limit(
    State(state): State<HttpState>,
    mut request: Request,
    next: Next,
) -> Result<Response, HttpError> {
    let identity = state
        .config
        .authenticate(request.headers())
        .ok_or(HttpError::Unauthorized)?;
    let permit = Arc::clone(&state.concurrency)
        .try_acquire_owned()
        .map_err(|_| HttpError::TooManyRequests)?;
    request.extensions_mut().insert(identity);
    let response = next.run(request).await;
    drop(permit);
    Ok(response)
}

async fn submit_task(
    State(state): State<HttpState>,
    Extension(identity): Extension<CallerIdentity>,
    request: Result<Json<ChatRequest>, JsonRejection>,
) -> Result<Json<ChatResponse>, HttpError> {
    // T-850-04/T1: mede construção/validação do Task (ver mapa T1–T6 no
    // topo do arquivo).
    let serialization_started = Instant::now();
    let mut task = validated_task(&state.config, identity, request?)?;
    let t_serialization_ns = serialization_started.elapsed().as_nanos() as u64;
    task.t_serialization_ns = t_serialization_ns;
    let task_id = task.task_id.clone();

    // EXP1b: despacho central emulado (quando --dispatch-mode).
    state.backend.dispatch_task(&mut task).await;

    // T-850-04/T2: mede o publish (write DDS; retorna após o dds_write).
    let transport_started = Instant::now();
    state.backend.publish_task(task).await.map_err(|error| {
        tracing::error!(%error, "HTTP task publication failed");
        HttpError::Internal
    })?;
    let t_transport_send_ns = transport_started.elapsed().as_nanos() as u64;

    tracing::info!(%task_id, "task published through HTTP boundary");
    Ok(Json(ChatResponse {
        task_id,
        status: "pending",
        t_serialization_ns,
        t_transport_send_ns,
    }))
}

async fn submit_task_sync(
    State(state): State<HttpState>,
    Extension(identity): Extension<CallerIdentity>,
    request: Result<Json<ChatRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, HttpError> {
    // T-850-04/T1 e T2: mesmas medidas do handler assíncrono (ver mapa
    // T1–T6 no topo do arquivo).
    let serialization_started = Instant::now();
    let mut task = validated_task(&state.config, identity, request?)?;
    let t_serialization_ns = serialization_started.elapsed().as_nanos() as u64;
    task.t_serialization_ns = t_serialization_ns;
    let task_id = task.task_id.clone();

    // EXP1b: despacho central emulado (quando --dispatch-mode).
    state.backend.dispatch_task(&mut task).await;

    let transport_started = Instant::now();
    state.backend.publish_task(task).await.map_err(|error| {
        tracing::error!(%error, "HTTP task publication failed");
        HttpError::Internal
    })?;
    let timings = PublishTimings {
        t_serialization_ns,
        t_transport_send_ns: transport_started.elapsed().as_nanos() as u64,
    };

    tokio::time::timeout(
        state.config.limits.dds_wait_timeout,
        wait_for_task(&state, &task_id, timings),
    )
    .await
    .map_err(|_| HttpError::GatewayTimeout)?
}

/// T-850-04: medidas da fase de publicação (T1/T2) que o loop de espera
/// precisa ecoar na resposta `/sync`.
struct PublishTimings {
    t_serialization_ns: u64,
    t_transport_send_ns: u64,
}

/// Instante atual em ns desde a época UNIX (mesma base de
/// `created_at_ns`/`completed_at_ns` das tasks).
fn wall_now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

async fn wait_for_task(
    state: &HttpState,
    task_id: &str,
    timings: PublishTimings,
) -> Result<Json<serde_json::Value>, HttpError> {
    loop {
        // T-850-04/T6: a duração da leitura que FECHAR o estado terminal é
        // a desserialização vista pelo cliente do `/sync` (leitura do cache
        // + clone do Task).
        let read_started = Instant::now();
        if let Some(current) = state.backend.read_task(task_id) {
            let t_deserialization_ns = read_started.elapsed().as_nanos() as u64;
            // T-850-04/T5: janela entre o agente publicar o estado terminal
            // (`completed_at_ns`, relógio de parede) e este poll observá-lo.
            // Skew de relógio (leitura antes do carimbo) → 0 honesto.
            let t_transport_return_ns = wall_now_ns().saturating_sub(current.completed_at_ns);
            match current.status {
                3 => {
                    // T-820-10: `finish_reason` é passado CRU do wire — com o
                    // agente publicando o vocabulário canônico
                    // (`orch_common::FinishReason`, ex. "COMPLETION"; fix
                    // T-820-09), o valor aqui é parseável por
                    // `FinishReason::parse`. `tokens_prompt` permanece 0 por
                    // contrato (adiamento deliberado da fase — ver
                    // specs/820-review-fixes/tasks.md: o campo não é
                    // propagado no IDL/TaskOutput no sistema inteiro).
                    return Ok(Json(json!({
                        "task_id": task_id,
                        "status": "completed",
                        "latency_ms": current.completed_at_ns.saturating_sub(current.created_at_ns) / 1_000_000,
                        "finish_reason": current.finish_reason,
                        "assigned_agent": current.assigned_agent,
                        "tokens_prompt": 0,
                        "tokens_completion": 0,
                        "t_serialization_ns": timings.t_serialization_ns,
                        "t_transport_send_ns": timings.t_transport_send_ns,
                        "t_agent_queue_ns": current.t_agent_queue_ns,
                        "t_inference_ns": current.t_inference_ns,
                        "t_transport_return_ns": t_transport_return_ns,
                        "t_deserialization_ns": t_deserialization_ns,
                    })));
                }
                4 => {
                    return Ok(Json(json!({
                        "task_id": task_id,
                        "status": "failed",
                        "error": current.finish_reason,
                        "t_serialization_ns": timings.t_serialization_ns,
                        "t_transport_send_ns": timings.t_transport_send_ns,
                        "t_agent_queue_ns": current.t_agent_queue_ns,
                        "t_inference_ns": current.t_inference_ns,
                        "t_transport_return_ns": t_transport_return_ns,
                        "t_deserialization_ns": t_deserialization_ns,
                    })));
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn validated_task(
    config: &HttpConfig,
    identity: CallerIdentity,
    Json(request): Json<ChatRequest>,
) -> Result<Task, HttpError> {
    if request.model.is_empty() || request.messages.is_empty() {
        return Err(HttpError::Unprocessable);
    }
    if !config.allowed_models.is_empty() && !config.allowed_models.contains(&request.model) {
        return Err(HttpError::Forbidden);
    }
    if request.messages.len() > config.limits.message_count {
        return Err(HttpError::Unprocessable);
    }
    let message_bytes = request.messages.iter().try_fold(0usize, |total, message| {
        total
            .checked_add(message.role.len())?
            .checked_add(message.content.len())
    });
    if message_bytes.is_none_or(|bytes| bytes > config.limits.message_bytes) {
        return Err(HttpError::PayloadTooLarge);
    }
    let max_tokens = request.max_tokens.unwrap_or(256);
    if max_tokens == 0 || max_tokens > config.limits.max_tokens {
        return Err(HttpError::Unprocessable);
    }
    let temperature = request.temperature.unwrap_or(0.7);
    if !temperature.is_finite() || !(-2.0..=2.0).contains(&temperature) {
        return Err(HttpError::Unprocessable);
    }
    let messages_json =
        serde_json::to_string(&request.messages).map_err(|_| HttpError::Internal)?;
    let task_id = uuid::Uuid::new_v4().to_string();
    let now_ns = wall_now_ns();

    // T-850-04: os campos t_* saem 0 aqui; T1 (`t_serialization_ns`) é
    // preenchido pelo handler ANTES do publish (viaja no Task), T2 é medido
    // em volta do publish e reportado na resposta HTTP, T3/T4 são do agente
    // e T5/T6 do cliente (ver mapa T1–T6 no topo do arquivo).
    Ok(Task {
        task_id,
        client_id: identity.0,
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: request.model,
        messages_json,
        temperature,
        max_tokens,
        stream: request.stream.unwrap_or(false),
        status: 0,
        priority: 5,
        created_at_ns: now_ns,
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: now_ns + 120_000_000_000,
        retry_count: 0,
        finish_reason: String::new(),
        t_serialization_ns: 0,
        t_transport_send_ns: 0,
        t_agent_queue_ns: 0,
        t_inference_ns: 0,
        t_transport_return_ns: 0,
        t_deserialization_ns: 0,
    })
}

impl From<JsonRejection> for HttpError {
    fn from(rejection: JsonRejection) -> Self {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::PayloadTooLarge
        } else {
            Self::Unprocessable
        }
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "component": "orchestrator",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn list_agents(State(state): State<HttpState>) -> Json<serde_json::Value> {
    let agents = state.backend.agents();
    let values: Vec<_> = agents
        .iter()
        .map(|agent| {
            json!({
                "agent_id": agent.agent_id,
                "hostname": agent.hostname,
                "model": agent.model,
                "specialization": agent.specialization,
                "slots_total": agent.slots_total,
                "slots_busy": agent.slots_busy,
                "vram_total_mb": agent.vram_total_mb,
                "vram_used_mb": agent.vram_used_mb,
                "ema_latency_ms": agent.ema_latency_ms,
                "completed_total": agent.completed_total,
                "failed_total": agent.failed_total,
                "health": agent.health,
                "last_update_ns": agent.last_update_ns,
                "uptime_seconds": agent.uptime_seconds,
            })
        })
        .collect();
    Json(json!({ "count": values.len(), "agents": values }))
}

#[cfg(feature = "dds")]
#[async_trait]
impl HttpBackend for crate::dds::OrchestratorDds {
    async fn publish_task(&self, task: Task) -> Result<(), HttpBackendError> {
        crate::dds::OrchestratorDds::publish_task(self, task)
            .await
            .map_err(|_| HttpBackendError)
    }

    /// EXP1b: despacho central emulado — delega ao modo configurado no
    /// orquestrador (`--dispatch-mode`): escolhe o agente menos ocupado e
    /// fixa `target_agent` (task segue PENDING; claim do agente permanece).
    async fn dispatch_task(&self, task: &mut Task) -> bool {
        crate::dds::OrchestratorDds::dispatch_task(self, task)
    }

    fn read_task(&self, task_id: &str) -> Option<Task> {
        self.dataspace()
            .caches()
            .read_task(task_id)
            .map(|task| (*task).clone())
    }

    fn agents(&self) -> Vec<AgentState> {
        self.registry().all()
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
