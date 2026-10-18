//! Handlers de chat/settings/VMs/Ansible/testes e paginas.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::server::{err, AppState};

#[derive(Deserialize)]
pub(crate) struct ChatBody {
    prompt: String,
    #[serde(default)]
    timeout_s: Option<f64>,
    #[serde(default)]
    model_required: Option<String>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    files: Vec<ChatFileBody>,
}

#[derive(Deserialize)]
struct ChatFileBody {
    name: String,
    mime_type: String,
    data_base64: String,
}

fn parse_spec(value: Option<&str>) -> crate::chat::Specialization {
    match value.unwrap_or("TEXT") {
        "VISION" => crate::chat::Specialization::Vision,
        "TRANSCRIPTION" => crate::chat::Specialization::Transcription,
        _ => crate::chat::Specialization::Text,
    }
}

/// Chat via DDS em SSE (paridade com a rota /api/chat/send do painel Python).
pub(crate) async fn chat_send(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChatBody>,
) -> Response {
    let spec = parse_spec(body.model_required.as_deref());
    let files: Vec<crate::chat::ChatFile> = body
        .files
        .iter()
        .map(|f| crate::chat::ChatFile {
            name: f.name.clone(),
            mime_type: f.mime_type.clone(),
            data_base64: f.data_base64.clone(),
        })
        .collect();
    let payload = match crate::chat::build_messages_json(&body.prompt, spec, &files) {
        Ok(payload) => payload,
        Err(e) => return err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()),
    };
    let _ = body.max_tokens;
    chat_event_stream(state, payload, body.timeout_s.unwrap_or(60.0)).await
}

pub(crate) async fn inventory_public(State(state): State<Arc<AppState>>) -> Json<Value> {
    let inventory = crate::inventory::load_inventory(&state.supervisor.config().ansible_inventory);
    Json(inventory.to_public())
}

pub(crate) async fn ansible_actions() -> Json<Value> {
    Json(json!(crate::ansible::list_actions()))
}

#[derive(Deserialize)]
pub(crate) struct AnsibleRunBody {
    action: String,
    limit: Option<String>,
}

pub(crate) async fn ansible_run(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnsibleRunBody>,
) -> Response {
    let cfg = state.supervisor.config();
    let inventory = crate::inventory::load_inventory(&cfg.ansible_inventory);
    match state.ansible.start(
        &body.action,
        body.limit.as_deref(),
        &inventory.secrets(),
        &inventory.all_limit_values(),
    ) {
        Ok(run) => Json(json!(run)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub(crate) async fn tests_list(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.tests.discover()))
}

#[derive(Deserialize)]
pub(crate) struct TestsRunBody {
    items: Vec<String>,
}

pub(crate) async fn tests_run(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TestsRunBody>,
) -> Response {
    match state.tests.run(&body.items) {
        Ok(record) => Json(json!(record)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, &e),
    }
}

pub(crate) async fn settings_get(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(crate::settings::settings_view(state.supervisor.config()))
}

pub(crate) async fn settings_post(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Response {
    let Some(object) = body.as_object() else {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "JSON objeto esperado");
    };
    match crate::settings::save_user_config(state.supervisor.config(), object) {
        Ok(saved) => Json(json!({"status": "ok", "saved": saved})).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

pub(crate) async fn chat_ready() -> Json<Value> {
    Json(json!({
        "ready": cfg!(feature = "dds"),
        "error": if cfg!(feature = "dds") { Value::Null } else { json!("build sem feature dds") },
    }))
}

/// Stream SSE do chat: eventos {type: token|final|error} + [DONE].
pub(crate) async fn chat_event_stream(
    state: Arc<AppState>,
    payload: String,
    timeout_s: f64,
) -> Response {
    #[cfg(feature = "dds")]
    let result = {
        let transport = crate::chat::ClientChatTransport::new(
            state.supervisor.config().default_dds_domain as u32,
            "control-panel-chat",
        );
        transport.send_async(&payload, timeout_s).await
    };
    #[cfg(not(feature = "dds"))]
    let result: Result<String, String> = {
        let _ = &payload;
        let _ = timeout_s;
        Err("build sem feature dds".into())
    };
    let _ = state;
    let events: Vec<String> = match result {
        Ok(content) => {
            let mut events: Vec<String> = content
                .chars()
                .collect::<Vec<_>>()
                .chunks(24)
                .map(|chunk| {
                    let text: String = chunk.iter().collect();
                    format!("data: {}\n\n", json!({"type": "token", "content": text}))
                })
                .collect();
            events.push(format!(
                "data: {}\n\n",
                json!({"type": "final", "content": content})
            ));
            events
        }
        Err(e) => vec![format!(
            "data: {}\n\n",
            json!({"type": "error", "message": e})
        )],
    };
    let stream = futures::stream::iter(
        events
            .into_iter()
            .chain(std::iter::once("data: [DONE]\n\n".to_string()))
            .map(Ok::<String, std::io::Error>),
    );
    axum::response::Response::builder()
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(axum::body::Body::from_stream(stream))
        .expect("resposta SSE")
}

#[derive(Deserialize)]
pub(crate) struct ChatStreamQuery {
    prompt: Option<String>,
    timeout_s: Option<f64>,
}

pub(crate) async fn chat_stream(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ChatStreamQuery>,
) -> Response {
    let prompt = q.prompt.unwrap_or_default();
    if prompt.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "Prompt ou arquivo obrigatorio");
    }
    let payload =
        match crate::chat::build_messages_json(&prompt, crate::chat::Specialization::Text, &[]) {
            Ok(payload) => payload,
            Err(e) => return err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()),
        };
    chat_event_stream(state, payload, q.timeout_s.unwrap_or(60.0)).await
}
