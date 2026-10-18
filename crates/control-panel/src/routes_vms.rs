//! Handlers de Cluster/VMs, Ansible, testes e páginas.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::server::{err, AppState};

pub(crate) async fn vms_hosts(State(state): State<Arc<AppState>>) -> Json<Value> {
    let inventory = crate::inventory::load_inventory(&state.supervisor.config().ansible_inventory);
    Json(inventory.to_public())
}

pub(crate) async fn vms_hosts_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let inventory = crate::inventory::load_inventory(&state.supervisor.config().ansible_inventory);
    Json(json!(state.host_status.get_all(&inventory.hosts)))
}

pub(crate) async fn vms_host_refresh(
    State(state): State<Arc<AppState>>,
    Path(host_name): Path<String>,
) -> Response {
    let inventory = crate::inventory::load_inventory(&state.supervisor.config().ansible_inventory);
    let Some(host) = inventory
        .hosts
        .iter()
        .find(|h| h.name == host_name || h.address == host_name)
    else {
        return err(StatusCode::NOT_FOUND, "host desconhecido");
    };
    Json(json!(state.host_status.get_one(host, true))).into_response()
}

pub(crate) async fn vms_groups(State(state): State<Arc<AppState>>) -> Json<Value> {
    let inventory = crate::inventory::load_inventory(&state.supervisor.config().ansible_inventory);
    Json(json!(inventory.groups))
}

pub(crate) async fn vms_runs(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.ansible.list_runs()))
}

pub(crate) async fn vms_run_get(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Response {
    match state.ansible.get_run(&run_id) {
        Some(run) => Json(json!(run)).into_response(),
        None => err(StatusCode::NOT_FOUND, "run desconhecida"),
    }
}

pub(crate) async fn vms_run_cancel(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Response {
    match state.ansible.cancel(&run_id) {
        Ok(run) => Json(json!(run)).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

/// SSE com as linhas novas da run até o estado terminal.
pub(crate) async fn vms_run_stream(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Response {
    let stream = futures::stream::unfold(
        (state.ansible.clone(), run_id, 0usize, false),
        |(ansible, id, mut sent, done)| async move {
            if done {
                return None;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let run = ansible.get_run(&id)?;
            let mut events = String::new();
            while sent < run.lines.len() {
                events.push_str(&format!(
                    "data: {}\n\n",
                    json!({"type": "line", "line": run.lines[sent]})
                ));
                sent += 1;
            }
            let terminal = matches!(run.status.as_str(), "finished" | "failed" | "cancelled");
            if terminal {
                events.push_str(&format!(
                    "data: {}\n\n",
                    json!({"type": "status", "status": run.status})
                ));
                events.push_str("data: [DONE]\n\n");
            }
            if events.is_empty() {
                events.push_str(": keep-alive\n\n");
            }
            Some((
                Ok::<String, std::io::Error>(events),
                (ansible, id, sent, terminal),
            ))
        },
    );
    axum::response::Response::builder()
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(axum::body::Body::from_stream(stream))
        .expect("resposta SSE")
}

#[derive(Deserialize)]
pub(crate) struct CategoryBody {
    pub category: String,
}

#[derive(Deserialize)]
pub(crate) struct PlaybookRunBody {
    pub limit: Option<String>,
}

pub(crate) async fn vms_playbook_run(
    State(state): State<Arc<AppState>>,
    Path(action_id): Path<String>,
    body: Option<Json<PlaybookRunBody>>,
) -> Response {
    let cfg = state.supervisor.config();
    let inventory = crate::inventory::load_inventory(&cfg.ansible_inventory);
    let limit = body.and_then(|Json(b)| b.limit);
    match state.ansible.start_async(
        &action_id,
        limit.as_deref(),
        &inventory.secrets(),
        &inventory.all_limit_values(),
    ) {
        Ok(run) => Json(json!(run)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub(crate) async fn tests_run_category(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CategoryBody>,
) -> Response {
    match state.tests.run_category(&body.category) {
        Ok(record) => Json(json!(record)).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, &e),
    }
}

pub(crate) async fn tests_run_get(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Response {
    match state.tests.get_run(&run_id) {
        Some(record) => Json(json!(record)).into_response(),
        None => err(StatusCode::NOT_FOUND, "run desconhecida"),
    }
}

pub(crate) async fn tests_history(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.tests.list_history(50)))
}

pub(crate) async fn page_dashboard() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("pages/dashboard.html"))
}
pub(crate) async fn page_services() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("pages/services.html"))
}
pub(crate) async fn page_tests() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("pages/tests.html"))
}
pub(crate) async fn page_chat() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("pages/chat.html"))
}
pub(crate) async fn page_vms() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("pages/vms.html"))
}
pub(crate) async fn page_settings() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("pages/settings.html"))
}
