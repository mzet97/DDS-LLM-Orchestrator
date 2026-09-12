//! Transporte HTTP localhost do nó (T-800-06, P2).
//!
//! Rotas reais sobre o [`OperationLog`]: versão, aplicação idempotente e
//! reconciliação. Erros do domínio viram status HTTP via `match` exaustivo —
//! nunca `500` genérico para falha prevista.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::operations::{NodeError, OpOutcome, OpRecord, OperationLog};
use crate::protocol::{AdminEnvelope, ProtocolError, ProtocolVersion, NODE_PROTOCOL_VERSION};

/// Estado compartilhado do servidor: versão anunciada + log de operações.
#[derive(Debug, Clone)]
pub struct NodeState {
    version: ProtocolVersion,
    log: Arc<Mutex<OperationLog>>,
}

impl NodeState {
    /// Estado inicial com os serviços próprios declarados.
    #[must_use]
    pub fn new(owned_services: Vec<String>) -> Self {
        Self {
            version: NODE_PROTOCOL_VERSION,
            log: Arc::new(Mutex::new(OperationLog::new(owned_services))),
        }
    }
}

/// Corpo de erro tipado do fio (código estável, mensagem humana).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiErrorBody>)>;

fn api_error(
    status: StatusCode,
    code: &'static str,
    message: String,
) -> (StatusCode, Json<ApiErrorBody>) {
    (status, Json(ApiErrorBody { code, message }))
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
    if let Err(err @ ProtocolError::Incompatible { .. }) = state.version.check(envelope.protocol) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "incompatible_protocol",
            err.to_string(),
        ));
    }
    let mut log = state.log.lock().await;
    match log.apply(envelope.operation_id, envelope.op) {
        Ok(OpOutcome::Applied(record)) => Ok(Json(ApplyResponse::Applied { record })),
        Ok(OpOutcome::AlreadyApplied(record)) => Ok(Json(ApplyResponse::AlreadyApplied { record })),
        Err(err) => Err(domain_error(err)),
    }
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
        .route("/operations/:id", get(get_operation))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AdminOp, OperationId};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use tower::ServiceExt;

    fn app() -> Router {
        router(NodeState::new(vec![String::from("dds-agent")]))
    }

    fn running(service: &str) -> AdminOp {
        AdminOp::SetService {
            service: String::from(service),
            running: true,
        }
    }

    fn post_apply(envelope: &AdminEnvelope) -> Request<Body> {
        Request::post("/apply")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(envelope).expect("envelope serializa"),
            ))
            .expect("request valido")
    }

    fn envelope(id: &str, op: AdminOp) -> AdminEnvelope {
        AdminEnvelope {
            protocol: NODE_PROTOCOL_VERSION,
            operation_id: OperationId(String::from(id)),
            op,
        }
    }

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .expect("corpo deve ser legivel");
        serde_json::from_slice(&bytes).expect("corpo deve ser JSON")
    }

    #[tokio::test]
    async fn version_endpoint_announces_node_protocol() {
        let response = app()
            .oneshot(
                Request::get("/version")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            body_json(response).await,
            serde_json::json!({"major": 1, "minor": 0})
        );
    }

    #[tokio::test]
    async fn repeated_apply_on_same_state_returns_already_applied() {
        let app = app();
        let first = app
            .clone()
            .oneshot(post_apply(&envelope("op-1", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(
            body_json(first).await["outcome"],
            serde_json::json!("applied")
        );

        let second = app
            .oneshot(post_apply(&envelope("op-1", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(
            body_json(second).await["outcome"],
            serde_json::json!("already_applied")
        );
    }

    #[tokio::test]
    async fn conflicting_payload_returns_409_without_overwrite() {
        let app = router(NodeState::new(vec![String::from("dds-agent")]));
        let stopped = AdminOp::SetService {
            service: String::from("dds-agent"),
            running: false,
        };
        let first = app
            .clone()
            .oneshot(post_apply(&envelope("op-2", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(first.status(), StatusCode::OK);

        let conflict = app
            .oneshot(post_apply(&envelope("op-2", stopped)))
            .await
            .expect("rota existe");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            body_json(conflict).await["code"],
            serde_json::json!("operation_id_conflict")
        );
    }

    #[tokio::test]
    async fn foreign_service_returns_403() {
        let response = app()
            .oneshot(post_apply(&envelope("op-3", running("postgres-alheio"))))
            .await
            .expect("rota existe");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            body_json(response).await["code"],
            serde_json::json!("out_of_scope")
        );
    }

    #[tokio::test]
    async fn incompatible_peer_protocol_returns_400() {
        let newer = AdminEnvelope {
            protocol: ProtocolVersion { major: 1, minor: 1 },
            operation_id: OperationId(String::from("op-4")),
            op: running("dds-agent"),
        };
        let response = app()
            .oneshot(post_apply(&newer))
            .await
            .expect("rota existe");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body_json(response).await["code"],
            serde_json::json!("incompatible_protocol")
        );
    }

    #[tokio::test]
    async fn reconcile_finds_applied_and_misses_unknown() {
        let app = app();
        let applied = app
            .clone()
            .oneshot(post_apply(&envelope("op-5", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(applied.status(), StatusCode::OK);

        let found = app
            .clone()
            .oneshot(
                Request::get("/operations/op-5")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");
        assert_eq!(found.status(), StatusCode::OK);

        let missing = app
            .oneshot(
                Request::get("/operations/inexistente")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body_json(missing).await["code"],
            serde_json::json!("unknown_operation")
        );
    }
}
