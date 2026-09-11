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
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

#[derive(Debug, thiserror::Error)]
#[error("DDS operation failed")]
pub struct HttpBackendError;

#[async_trait]
pub trait HttpBackend: Send + Sync {
    async fn publish_task(&self, task: Task) -> Result<(), HttpBackendError>;
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
    let task = validated_task(&state.config, identity, request?)?;
    let task_id = task.task_id.clone();
    state.backend.publish_task(task).await.map_err(|error| {
        tracing::error!(%error, "HTTP task publication failed");
        HttpError::Internal
    })?;
    tracing::info!(%task_id, "task published through HTTP boundary");
    Ok(Json(ChatResponse {
        task_id,
        status: "pending",
    }))
}

async fn submit_task_sync(
    State(state): State<HttpState>,
    Extension(identity): Extension<CallerIdentity>,
    request: Result<Json<ChatRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, HttpError> {
    let task = validated_task(&state.config, identity, request?)?;
    let task_id = task.task_id.clone();
    state.backend.publish_task(task).await.map_err(|error| {
        tracing::error!(%error, "HTTP task publication failed");
        HttpError::Internal
    })?;

    tokio::time::timeout(
        state.config.limits.dds_wait_timeout,
        wait_for_task(&state, &task_id),
    )
    .await
    .map_err(|_| HttpError::GatewayTimeout)?
}

async fn wait_for_task(
    state: &HttpState,
    task_id: &str,
) -> Result<Json<serde_json::Value>, HttpError> {
    loop {
        if let Some(current) = state.backend.read_task(task_id) {
            match current.status {
                3 => {
                    return Ok(Json(json!({
                        "task_id": task_id,
                        "status": "completed",
                        "latency_ms": current.completed_at_ns.saturating_sub(current.created_at_ns) / 1_000_000,
                        "finish_reason": current.finish_reason,
                        "assigned_agent": current.assigned_agent,
                        "tokens_prompt": 0,
                        "tokens_completion": 0,
                    })));
                }
                4 => {
                    return Ok(Json(json!({
                        "task_id": task_id,
                        "status": "failed",
                        "error": current.finish_reason,
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
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

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
