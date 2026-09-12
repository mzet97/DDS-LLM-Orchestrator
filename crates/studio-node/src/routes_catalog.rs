//! Rotas do catálogo compartilhado: autoridade condicional com journal
//! (P2a; G-44/45/47/49/57).

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::catalog_auth::AuthorityError;
use crate::server::{api_error, api_error_details, ApiErrorBody, ApiResult, NodeState};

fn authority_error(err: AuthorityError) -> (StatusCode, Json<ApiErrorBody>) {
    match err {
        AuthorityError::Conflict { current } => api_error_details(
            StatusCode::CONFLICT,
            "revision_conflict",
            format!("base obsoleta; vigente: {current:?}"),
            serde_json::json!({"current": current}),
        ),
        AuthorityError::Tombstoned { deleted_at } => api_error(
            StatusCode::GONE,
            "tombstoned",
            format!("id removido em {deleted_at}; recriar exige identidade nova"),
        ),
        AuthorityError::CursorExpired => api_error(
            StatusCode::GONE,
            "cursor_expired",
            String::from("cursor fora da retenção; refazer snapshot"),
        ),
        AuthorityError::Corrupt { path, detail } => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "catalog_corrupt",
            format!("journal {path}: {detail}"),
        ),
    }
}

/// Corpo de publicação condicional (G-47/57).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PublishBody {
    id: String,
    base: Option<u64>,
    value: String,
    generation: u64,
}

/// Corpo de exclusão condicional (G-57).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DeleteBody {
    id: String,
    base: u64,
}

/// Snapshot consistente do catálogo compartilhado (§34.8).
pub(crate) async fn catalog_snapshot(
    State(state): State<NodeState>,
) -> Json<studio_core::catalog::Snapshot> {
    Json(state.catalog.lock().await.snapshot())
}

/// Eventos contíguos desde `?since=` (G-49).
pub(crate) async fn catalog_events(
    State(state): State<NodeState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, u64>>,
) -> ApiResult<Vec<studio_core::catalog::Event>> {
    let since = params.get("since").copied().unwrap_or(0);
    state
        .catalog
        .lock()
        .await
        .events_since(since)
        .map(Json)
        .map_err(authority_error)
}

/// Publicação condicional no catálogo compartilhado (G-47/57).
pub(crate) async fn catalog_publish(
    State(state): State<NodeState>,
    Json(body): Json<PublishBody>,
) -> ApiResult<RevisionOut> {
    state
        .catalog
        .lock()
        .await
        .publish(body.id, body.base, body.value, body.generation)
        .map(|revision| Json(RevisionOut { revision }))
        .map_err(authority_error)
}

/// Exclusão condicional com tombstone (G-57).
pub(crate) async fn catalog_delete(
    State(state): State<NodeState>,
    Json(body): Json<DeleteBody>,
) -> ApiResult<RevisionOut> {
    state
        .catalog
        .lock()
        .await
        .delete(body.id, body.base)
        .map(|revision| Json(RevisionOut { revision }))
        .map_err(authority_error)
}

/// Revisão resultante de publicação/exclusão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionOut {
    pub revision: u64,
}
