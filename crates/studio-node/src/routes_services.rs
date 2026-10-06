//! Rotas de serviços próprios: plano pretendido x efetivo e atuação
//! idempotente (RF-05; G-05/06/07).
//!
//! As chamadas de probe/atuador rodam `systemctl` (bloqueante) — são sempre
//! despachadas via `tokio::task::spawn_blocking` e NUNCA sob o Mutex do log
//! de operações (REQ/T-820-18): acquire → log → release → atuar → acquire →
//! persistir. Segurar o lock async durante I/O de SO trava todo o nó
//! (`/apply`, `/operations`) atrás de um subprocesso.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::actuator::ActuatorError;
use crate::server::{api_error, domain_error, ApiResult, NodeState};
use std::sync::Arc;

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

/// Sonda o estado real FORA da thread async (systemctl é bloqueante).
async fn probe_active(
    probe: &Arc<dyn crate::probe::Probe>,
    service: &str,
) -> Result<bool, (StatusCode, Json<crate::server::ApiErrorBody>)> {
    let probe = Arc::clone(probe);
    let service = service.to_string();
    tokio::task::spawn_blocking(move || probe.is_active(&service))
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                format!("sonda falhou: {err}"),
            )
        })
}

/// Atua FORA da thread async (systemctl é bloqueante).
async fn actuate_blocking(
    actuator: &Arc<dyn crate::actuator::Actuator>,
    service: &str,
    wanted: bool,
) -> Result<(), (StatusCode, Json<crate::server::ApiErrorBody>)> {
    let actuator = Arc::clone(actuator);
    let service = service.to_string();
    let result = tokio::task::spawn_blocking(move || actuator.set_active(&service, wanted))
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                format!("atuador despachou falha: {err}"),
            )
        })?;
    result.map_err(|ActuatorError::Failed { service, detail }| {
        api_error(
            StatusCode::BAD_GATEWAY,
            "actuator_failed",
            format!("atuador falhou em {service}: {detail}"),
        )
    })
}

/// Efetiva start/stop em serviço próprio com idempotência por id (RF-05).
///
/// Registra a intenção, age só se houver divergência e persiste. Repetir o
/// mesmo `operation_id` já convergido não reexecuta (`acted: false`).
///
/// Ordem do lock (REQ/T-820-18): o Mutex do log é tomado duas vezes em
/// seções curtas (log da intenção; persistência) — as chamadas bloqueantes
/// de probe/atuador rodam SEM o lock, em `spawn_blocking`.
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
    // 1. Log da intenção (lock curto; solta antes de qualquer I/O de SO).
    let applied = {
        let mut log = state.log.lock().await;
        log.apply(body.operation_id, op)
    };
    if let Err(err) = applied {
        return Err(domain_error(err));
    }
    // 2. Sonda/ato sem o lock do log, fora da thread async.
    let mut active = probe_active(&state.probe, &service).await?;
    let mut acted = false;
    if active != wanted {
        actuate_blocking(&state.actuator, &service, wanted).await?;
        acted = true;
        active = probe_active(&state.probe, &service).await?;
    }
    // 3. Persistência (lock curto de novo).
    if let Some(storage) = state.storage.clone() {
        let saved = {
            let log = state.log.lock().await;
            storage.save(&log)
        };
        if let Err(err) = saved {
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
///
/// O plano (log) é lido sob lock curto; os probes bloqueantes rodam depois,
/// sem o lock e fora da thread async (REQ/T-820-18).
pub(crate) async fn list_services(State(state): State<NodeState>) -> ApiResult<Vec<ServiceStatus>> {
    let plan: Vec<(String, Option<bool>)> = {
        let log = state.log.lock().await;
        log.owned_services()
            .iter()
            .map(|service| (service.clone(), log.wanted(service)))
            .collect()
    };
    let mut rows = Vec::with_capacity(plan.len());
    for (service, wanted) in plan {
        let active = probe_active(&state.probe, &service).await?;
        rows.push(ServiceStatus {
            service,
            wanted,
            active,
        });
    }
    rows.sort_by(|a, b| a.service.cmp(&b.service));
    Ok(Json(rows))
}
