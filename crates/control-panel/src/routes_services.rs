//! Handlers de servicos/catalogo/presets/dashboard/hardware.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::catalog::{get_service_catalog, OverrideValue};
use crate::hardware::hardware_summary;
use crate::presets::get_presets;
use crate::server::{err, AppState};

pub(crate) async fn status_all(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.supervisor.list_status()))
}

pub(crate) async fn service_status(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Json<Value> {
    Json(json!(state.supervisor.status(&name)))
}

#[derive(Deserialize, Default)]
pub(crate) struct OverridesBody {
    #[serde(default, flatten)]
    overrides: BTreeMap<String, OverrideValue>,
}

pub(crate) async fn service_start(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Option<Json<OverridesBody>>,
) -> Response {
    let catalog = get_service_catalog(state.supervisor.config());
    let Some(definition) = catalog.get(&name) else {
        return err(StatusCode::NOT_FOUND, "Servico nao encontrado no catalogo");
    };
    let overrides = body.map(|Json(b)| b.overrides).unwrap_or_default();
    match state.supervisor.start_service(definition, &overrides) {
        Ok(status) => Json(json!(status)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn service_stop(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    match state.supervisor.stop_service(&name) {
        Ok(status) => Json(json!(status)).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

pub(crate) async fn service_restart(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    match state.supervisor.restart_service(&name) {
        Ok(status) => Json(json!(status)).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub(crate) struct LinesQuery {
    lines: Option<usize>,
}

pub(crate) async fn service_logs(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(q): Query<LinesQuery>,
) -> Json<Value> {
    Json(json!({"name": name, "lines": state.supervisor.get_logs(&name, q.lines.unwrap_or(100))}))
}

pub(crate) async fn service_clear_logs(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    match state.supervisor.clear_logs(&name) {
        Ok(()) => Json(json!({"name": name, "cleared": true})).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

pub(crate) async fn stop_all(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.supervisor.stop_all();
    Json(json!({"stopped": true}))
}

#[derive(Deserialize)]
pub(crate) struct AgentsBody {
    count: Option<usize>,
    engine: Option<String>,
    slots: Option<i64>,
}

pub(crate) async fn agents_start(
    State(state): State<Arc<AppState>>,
    body: Option<Json<AgentsBody>>,
) -> Response {
    let body = body.map(|Json(b)| b).unwrap_or(AgentsBody {
        count: None,
        engine: None,
        slots: None,
    });
    match state.supervisor.start_agents(
        body.count.unwrap_or(1),
        body.engine.as_deref().unwrap_or("dds"),
        body.slots.unwrap_or(1),
    ) {
        Ok(statuses) => Json(json!(statuses)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub(crate) async fn agents_stop_all(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({"stopped": state.supervisor.stop_all_agents()}))
}

pub(crate) async fn presets_list() -> Json<Value> {
    Json(json!(get_presets()
        .iter()
        .map(|p| json!({"id": p.id, "name": p.name, "description": p.description}))
        .collect::<Vec<_>>()))
}

pub(crate) async fn preset_apply(
    State(state): State<Arc<AppState>>,
    Path(preset_id): Path<String>,
) -> Response {
    match state.supervisor.apply_preset(&preset_id) {
        Ok(statuses) => Json(json!(statuses)).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

pub(crate) async fn catalog_list(State(state): State<Arc<AppState>>) -> Json<Value> {
    let catalog = get_service_catalog(state.supervisor.config());
    Json(json!(catalog
        .values()
        .map(|s| json!({
            "name": s.name,
            "display_name": s.display_name,
            "description": s.description,
            "category": s.category,
            "depends_on": s.depends_on,
            "tags": s.tags,
            "overridable_args": s.overridable_args,
        }))
        .collect::<Vec<_>>()))
}

pub(crate) async fn dashboard(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(state.supervisor.dashboard_summary())
}

pub(crate) async fn hardware() -> Json<Value> {
    Json(hardware_summary())
}
