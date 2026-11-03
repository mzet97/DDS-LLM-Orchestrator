//! Transporte HTTP localhost do nó (T-800-06, P2).
//!
//! Rotas reais sobre o [`OperationLog`]: versão, aplicação idempotente e
//! reconciliação. Erros do domínio viram status HTTP via `match` exaustivo —
//! nunca `500` genérico para falha prevista.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
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
    /// T-890-05: persistência atrás de trait (JSON compat / SQLite default).
    pub(crate) storage: Option<Arc<dyn crate::storage::Storage>>,
    pub(crate) probe: Arc<dyn Probe>,
    pub(crate) actuator: Arc<dyn Actuator>,
    /// Serializa sonda/atuação entre requisições concorrentes (P1): sem ela,
    /// start × stop simultâneos para o mesmo serviço atuavam independentes e
    /// a ordem de atuação podia contradizer a última intenção registrada. A
    /// releitura da intenção é uma seção curta do log dentro desta trava (o
    /// lock do log nunca é segurado durante I/O de SO — REQ/T-820-18).
    pub(crate) actuate_lock: Arc<Mutex<()>>,
    pub(crate) catalog: Arc<Mutex<CatalogAuthority>>,
    /// T-840-01: SHA-256 do token de acesso (`STUDIO_NODE_TOKEN`). `None`
    /// desliga a autenticação — só aceitável com bind em 127.0.0.1.
    pub(crate) token_hash: Option<[u8; 32]>,
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
            storage: None,
            probe,
            actuator,
            actuate_lock: Arc::new(Mutex::new(())),
            catalog: Arc::new(Mutex::new(CatalogAuthority::new())),
            token_hash: None,
        }
    }

    /// Configura o token de acesso (T-840-01): todas as rotas, exceto
    /// `GET /version`, passam a exigir `Authorization: Bearer <token>`
    /// (comparação em tempo constante sobre o SHA-256 de ambos os lados).
    #[must_use]
    pub fn with_token(mut self, token: &str) -> Self {
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        self.token_hash = Some(digest);
        self
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
    /// Estado com persistência no caminho dado: extensão `.json` usa o
    /// snapshot JSON (compat T-830-05); qualquer outro caminho usa o SQLite
    /// relacional (default T-890-05, com migração não destrutiva do JSON).
    pub fn with_db(db_path: PathBuf, owned_services: Vec<String>) -> Result<Self, NodeError> {
        let storage: Arc<dyn crate::storage::Storage> =
            if db_path.extension().is_some_and(|ext| ext == "json") {
                Arc::new(crate::storage::JsonStorage::new(db_path))
            } else {
                Arc::new(crate::storage::SqliteStorage::new(db_path))
            };
        Self::with_storage(storage, owned_services)
    }

    /// T-890-05: estado com persistência atrás do trait `Storage` — carrega
    /// o log existente (com migração JSON→SQLite quando couber) ou começa
    /// novo no primeiro boot; persistência corrompida falha rápido.
    pub fn with_storage(
        storage: Arc<dyn crate::storage::Storage>,
        owned_services: Vec<String>,
    ) -> Result<Self, NodeError> {
        let log = match storage.load()? {
            Some(log) => log,
            None => OperationLog::new(owned_services),
        };
        let catalog = CatalogAuthority::with_journal(Self::journal_path(storage.path()))
            .map_err(|err| NodeError::Storage(format!("catalogo: {err}")))?;
        Ok(Self {
            version: NODE_PROTOCOL_VERSION,
            log: Arc::new(Mutex::new(log)),
            storage: Some(storage),
            probe: Arc::new(SystemdProbe),
            actuator: Arc::new(SystemdActuator),
            actuate_lock: Arc::new(Mutex::new(())),
            catalog: Arc::new(Mutex::new(catalog)),
            token_hash: None,
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

mod server_routes;

pub use server_routes::router;
