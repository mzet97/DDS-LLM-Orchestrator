//! Parte de `server` (extraída para manter os arquivos revisáveis).

use super::*;

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
    if let Some(storage) = &state.storage {
        if let Err(err) = storage.save(&log) {
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

/// Middleware de autenticação (T-840-01): com token configurado, exige
/// `Authorization: Bearer <token>` — comparação em tempo constante sobre o
/// SHA-256 de ambos os lados (o hash nunca deixa o processo). Sem token
/// configurado, passa direto (modo localhost).
pub(crate) async fn require_token(
    State(state): State<NodeState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.token_hash else {
        return next.run(request).await;
    };
    let provided = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    let digest: [u8; 32] = Sha256::digest(provided.as_bytes()).into();
    if expected.ct_eq(&digest).into() {
        return next.run(request).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiErrorBody {
            code: "unauthorized",
            message: String::from("token de acesso ausente ou invalido"),
            details: None,
        }),
    )
        .into_response()
}

/// Roteador do nó com o estado compartilhado. `GET /version` fica aberto
/// (sonda/negociação de protocolo); todo o restante passa pelo middleware de
/// token quando configurado (T-840-01).
pub fn router(state: NodeState) -> Router {
    let protected = Router::new()
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
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_token,
        ));
    Router::new()
        .route("/version", get(get_version))
        .merge(protected)
        .with_state(state)
}
