//! Transporte HTTP localhost do nó (T-800-06, P2).
//!
//! Rotas reais sobre o [`OperationLog`]: versão, aplicação idempotente e
//! reconciliação. Erros do domínio viram status HTTP via `match` exaustivo —
//! nunca `500` genérico para falha prevista.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::actuator::{Actuator, SystemdActuator};
use crate::catalog_auth::CatalogAuthority;

use crate::operations::{NodeError, OpOutcome, OpRecord, OperationLog};
use crate::probe::{Probe, SystemdProbe};
use crate::protocol::{AdminEnvelope, ProtocolError, ProtocolVersion, NODE_PROTOCOL_VERSION};
pub use crate::routes_catalog::RevisionOut;
pub use crate::routes_services::{ActuateOut, ServiceStatus};

/// Estado compartilhado do servidor: versão anunciada + log de operações,
/// com caminho opcional de persistência (P2: operações persistidas) e sonda
/// de estado efetivo (base do plano/diff, sem efeitos).
#[derive(Clone)]
pub struct NodeState {
    pub(crate) version: ProtocolVersion,
    pub(crate) log: Arc<Mutex<OperationLog>>,
    pub(crate) db_path: Option<PathBuf>,
    pub(crate) probe: Arc<dyn Probe>,
    pub(crate) actuator: Arc<dyn Actuator>,
    pub(crate) catalog: Arc<Mutex<CatalogAuthority>>,
}

impl NodeState {
    /// Estado inicial com os serviços próprios declarados (sem persistência).
    #[must_use]
    pub fn new(owned_services: Vec<String>) -> Self {
        Self::with_parts(
            owned_services,
            Arc::new(SystemdProbe),
            Arc::new(SystemdActuator),
        )
    }

    /// Estado com sonda explícita (testes usam `FakeProbe`).
    #[must_use]
    pub fn with_probe(owned_services: Vec<String>, probe: Arc<dyn Probe>) -> Self {
        Self::with_parts(owned_services, probe, Arc::new(SystemdActuator))
    }

    /// Estado com sonda e atuador explícitos (testes).
    #[must_use]
    pub fn with_parts(
        owned_services: Vec<String>,
        probe: Arc<dyn Probe>,
        actuator: Arc<dyn Actuator>,
    ) -> Self {
        Self {
            version: NODE_PROTOCOL_VERSION,
            log: Arc::new(Mutex::new(OperationLog::new(owned_services))),
            db_path: None,
            probe,
            actuator,
            catalog: Arc::new(Mutex::new(CatalogAuthority::new())),
        }
    }

    /// Caminho do journal do catálogo derivado do DB de operações.
    fn journal_path(db_path: &std::path::Path) -> PathBuf {
        let stem = db_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("studio-node");
        db_path.with_file_name(format!("{stem}.catalog.jsonl"))
    }

    /// Estado com persistência: carrega o log existente ou começa novo no
    /// primeiro boot; arquivo corrompido falha rápido em vez de mascarar.
    pub fn with_db(db_path: PathBuf, owned_services: Vec<String>) -> Result<Self, NodeError> {
        let log = match OperationLog::load(&db_path) {
            Ok(log) => log,
            Err(NodeError::Storage(_)) if !db_path.exists() => OperationLog::new(owned_services),
            Err(err) => return Err(err),
        };
        let catalog = CatalogAuthority::with_journal(Self::journal_path(&db_path))
            .map_err(|err| NodeError::Storage(format!("catalogo: {err}")))?;
        Ok(Self {
            version: NODE_PROTOCOL_VERSION,
            log: Arc::new(Mutex::new(log)),
            db_path: Some(db_path),
            probe: Arc::new(SystemdProbe),
            actuator: Arc::new(SystemdActuator),
            catalog: Arc::new(Mutex::new(catalog)),
        })
    }
}

/// Corpo de erro tipado do fio (código estável, mensagem humana e detalhe
/// estruturado opcional — aditivo: clientes antigos ignoram `details`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

pub(crate) type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiErrorBody>)>;

pub(crate) fn api_error(
    status: StatusCode,
    code: &'static str,
    message: String,
) -> (StatusCode, Json<ApiErrorBody>) {
    (
        status,
        Json(ApiErrorBody {
            code,
            message,
            details: None,
        }),
    )
}

pub(crate) fn api_error_details(
    status: StatusCode,
    code: &'static str,
    message: String,
    details: serde_json::Value,
) -> (StatusCode, Json<ApiErrorBody>) {
    (
        status,
        Json(ApiErrorBody {
            code,
            message,
            details: Some(details),
        }),
    )
}

pub(crate) fn domain_error(err: NodeError) -> (StatusCode, Json<ApiErrorBody>) {
    match err {
        NodeError::OperationIdConflict => api_error(
            StatusCode::CONFLICT,
            "operation_id_conflict",
            err.to_string(),
        ),
        NodeError::OutOfScope(service) => api_error(
            StatusCode::FORBIDDEN,
            "out_of_scope",
            format!("servico fora do escopo do no: {service}"),
        ),
        NodeError::Storage(detail) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "storage_error",
            format!("falha de persistencia: {detail}"),
        ),
    }
}

/// Resposta de `/apply`: o que aconteceu com a operação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ApplyResponse {
    Applied { record: OpRecord },
    AlreadyApplied { record: OpRecord },
}

async fn get_version(State(state): State<NodeState>) -> Json<ProtocolVersion> {
    Json(state.version)
}

async fn post_apply(
    State(state): State<NodeState>,
    Json(envelope): Json<AdminEnvelope>,
) -> ApiResult<ApplyResponse> {
    match state.version.check(envelope.protocol) {
        Ok(()) => {}
        Err(err @ ProtocolError::Incompatible { .. }) => {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "incompatible_protocol",
                err.to_string(),
            ));
        }
    }
    let mut log = state.log.lock().await;
    let outcome = match log.apply(envelope.operation_id, envelope.op) {
        Ok(OpOutcome::Applied(record)) => ApplyResponse::Applied { record },
        Ok(OpOutcome::AlreadyApplied(record)) => ApplyResponse::AlreadyApplied { record },
        Err(err) => return Err(domain_error(err)),
    };
    if let Some(path) = &state.db_path {
        if let Err(err) = log.save(path) {
            return Err(domain_error(err));
        }
    }
    Ok(Json(outcome))
}

async fn list_operations(State(state): State<NodeState>) -> Json<Vec<OpRecord>> {
    let log = state.log.lock().await;
    let mut records: Vec<OpRecord> = log.records().cloned().collect();
    records.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    Json(records)
}

async fn get_operation(
    State(state): State<NodeState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult<OpRecord> {
    let log = state.log.lock().await;
    match log.reconcile(&crate::protocol::OperationId(id)) {
        Some(record) => Ok(Json(record.clone())),
        None => Err(api_error(
            StatusCode::NOT_FOUND,
            "unknown_operation",
            String::from("operation_id desconhecido"),
        )),
    }
}

/// Roteador do nó com o estado compartilhado.
pub fn router(state: NodeState) -> Router {
    Router::new()
        .route("/version", get(get_version))
        .route("/apply", axum::routing::post(post_apply))
        .route("/operations", get(list_operations))
        .route("/services", get(crate::routes_services::list_services))
        .route(
            "/services/:service/:action",
            axum::routing::post(crate::routes_services::actuate_service),
        )
        .route("/operations/:id", get(get_operation))
        .route(
            "/catalog/snapshot",
            get(crate::routes_catalog::catalog_snapshot),
        )
        .route(
            "/catalog/events",
            get(crate::routes_catalog::catalog_events),
        )
        .route(
            "/catalog/publish",
            axum::routing::post(crate::routes_catalog::catalog_publish),
        )
        .route(
            "/catalog/delete",
            axum::routing::post(crate::routes_catalog::catalog_delete),
        )
        .with_state(state)
}
