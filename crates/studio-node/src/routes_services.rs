//! Rotas de serviços próprios: plano pretendido x efetivo e atuação
//! idempotente (RF-05; G-05/06/07).

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::actuator::ActuatorError;
use crate::server::{api_error, domain_error, ApiResult, NodeState};

/// Serviço próprio: pretendido (log) × efetivo (gerenciador). Divergência é
/// o diff legível do plano — este endpoint nunca altera o host (G-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: String,
    pub wanted: Option<bool>,
    pub active: bool,
}
/// Corpo da atuação idempotente (G-05/06: repetir o id não duplica efeito).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ActuateBody {
    operation_id: crate::protocol::OperationId,
}

/// Desfecho da atuação: pretendido, efetivo e se houve efeito novo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActuateOut {
    pub service: String,
    pub wanted: bool,
    pub active: bool,
    pub acted: bool,
}

/// Efetiva start/stop em serviço próprio com idempotência por id (RF-05).
///
/// Registra a intenção, age só se houver divergência e persiste. Repetir o
/// mesmo `operation_id` já convergido não reexecuta (`acted: false`).
pub(crate) async fn actuate_service(
    State(state): State<NodeState>,
    axum::extract::Path((service, action)): axum::extract::Path<(String, String)>,
    Json(body): Json<ActuateBody>,
) -> ApiResult<ActuateOut> {
    let wanted = match action.as_str() {
        "start" => true,
        "stop" => false,
        _ => {
            return Err(api_error(
                StatusCode::NOT_FOUND,
                "unknown_action",
                format!("ação desconhecida: {action} (use start|stop)"),
            ));
        }
    };
    let op = crate::protocol::AdminOp::SetService {
        service: service.clone(),
        running: wanted,
    };
    let mut log = state.log.lock().await;
    if let Err(err) = log.apply(body.operation_id, op) {
        return Err(domain_error(err));
    }
    let mut active = state.probe.is_active(&service);
    let mut acted = false;
    if active != wanted {
        if let Err(err) = state.actuator.set_active(&service, wanted) {
            let ActuatorError::Failed { service, detail } = err;
            return Err(api_error(
                StatusCode::BAD_GATEWAY,
                "actuator_failed",
                format!("atuador falhou em {service}: {detail}"),
            ));
        }
        acted = true;
        active = state.probe.is_active(&service);
    }
    if let Some(path) = &state.db_path {
        if let Err(err) = log.save(path) {
            return Err(domain_error(err));
        }
    }
    Ok(Json(ActuateOut {
        service,
        wanted,
        active,
        acted,
    }))
}

/// Plano legível: pretendido × efetivo por serviço próprio. Só lê (G-07).
pub(crate) async fn list_services(State(state): State<NodeState>) -> Json<Vec<ServiceStatus>> {
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
