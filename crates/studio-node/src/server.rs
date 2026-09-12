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

use crate::catalog_auth::{AuthorityError, CatalogAuthority};
use crate::operations::{NodeError, OpOutcome, OpRecord, OperationLog};
use crate::probe::{Probe, SystemdProbe};
use crate::protocol::{AdminEnvelope, ProtocolError, ProtocolVersion, NODE_PROTOCOL_VERSION};

/// Estado compartilhado do servidor: versão anunciada + log de operações,
/// com caminho opcional de persistência (P2: operações persistidas) e sonda
/// de estado efetivo (base do plano/diff, sem efeitos).
#[derive(Clone)]
pub struct NodeState {
    version: ProtocolVersion,
    log: Arc<Mutex<OperationLog>>,
    db_path: Option<PathBuf>,
    probe: Arc<dyn Probe>,
    catalog: Arc<Mutex<CatalogAuthority>>,
}

impl NodeState {
    /// Estado inicial com os serviços próprios declarados (sem persistência).
    #[must_use]
    pub fn new(owned_services: Vec<String>) -> Self {
        Self::with_probe(owned_services, Arc::new(SystemdProbe))
    }

    /// Estado com sonda explícita (testes usam `FakeProbe`).
    #[must_use]
    pub fn with_probe(owned_services: Vec<String>, probe: Arc<dyn Probe>) -> Self {
        Self {
            version: NODE_PROTOCOL_VERSION,
            log: Arc::new(Mutex::new(OperationLog::new(owned_services))),
            db_path: None,
            probe,
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
            catalog: Arc::new(Mutex::new(catalog)),
        })
    }
}

/// Serviço próprio: pretendido (log) × efetivo (gerenciador). Divergência é
/// o diff legível do plano — este endpoint nunca altera o host (G-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: String,
    pub wanted: Option<bool>,
    pub active: bool,
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

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiErrorBody>)>;

fn api_error(
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

fn api_error_details(
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

fn domain_error(err: NodeError) -> (StatusCode, Json<ApiErrorBody>) {
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

/// Plano legível: pretendido × efetivo por serviço próprio. Só lê (G-07).
async fn list_services(State(state): State<NodeState>) -> Json<Vec<ServiceStatus>> {
    let log = state.log.lock().await;
    let mut rows: Vec<ServiceStatus> = log
        .owned_services()
        .iter()
        .map(|service| ServiceStatus {
            service: service.clone(),
            wanted: log.wanted(service),
            active: state.probe.is_active(service),
        })
        .collect();
    rows.sort_by(|a, b| a.service.cmp(&b.service));
    Json(rows)
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
struct PublishBody {
    id: String,
    base: Option<u64>,
    value: String,
    generation: u64,
}

/// Corpo de exclusão condicional (G-57).
#[derive(Debug, Clone, Deserialize)]
struct DeleteBody {
    id: String,
    base: u64,
}

/// Snapshot consistente do catálogo compartilhado (§34.8).
async fn catalog_snapshot(State(state): State<NodeState>) -> Json<studio_core::catalog::Snapshot> {
    Json(state.catalog.lock().await.snapshot())
}

/// Eventos contíguos desde `?since=` (G-49).
async fn catalog_events(
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
async fn catalog_publish(
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
async fn catalog_delete(
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

/// Roteador do nó com o estado compartilhado.
pub fn router(state: NodeState) -> Router {
    Router::new()
        .route("/version", get(get_version))
        .route("/apply", axum::routing::post(post_apply))
        .route("/operations", get(list_operations))
        .route("/services", get(list_services))
        .route("/operations/:id", get(get_operation))
        .route("/catalog/snapshot", get(catalog_snapshot))
        .route("/catalog/events", get(catalog_events))
        .route("/catalog/publish", axum::routing::post(catalog_publish))
        .route("/catalog/delete", axum::routing::post(catalog_delete))
        .with_state(state)
}
