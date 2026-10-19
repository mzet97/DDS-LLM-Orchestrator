//! Serviços do nó no Studio: plano legível (`GET /services`).
//!
//! Reutiliza `ServiceStatus` do `studio-node`: pretendido × efetivo.
//! Divergência é diff, nunca efeito — este painel não altera o host.
//!
//! Chamadas HTTP bloqueantes (até 30 s na atuação) rodam em THREAD de
//! trabalho (padrão `models.rs`: thread + mpsc + `poll` por frame) — a
//! thread de UI nunca bloqueia (REQ/T-820-19).

use std::sync::mpsc;
use studio_node::server::{ActuateOut, ServiceStatus};
use thiserror::Error;

/// Erros da leitura de serviços.
#[derive(Debug, Error)]
pub enum ServicesError {
    /// Nó inalcançável ou fora do contrato.
    #[error("falha ao ler servicos em {url}: {detail}")]
    Unreachable { url: String, detail: String },
}

/// Lista os serviços próprios com pretendido × efetivo.
pub fn list_services(base_url: &str) -> Result<Vec<ServiceStatus>, ServicesError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    client
        .get(format!("{url}/services"))
        .send()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .error_for_status()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .json::<Vec<ServiceStatus>>()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

/// Efetiva start/stop com idempotência por operação (RF-05).
pub fn actuate(
    base_url: &str,
    service: &str,
    start: bool,
    operation_id: &str,
) -> Result<ActuateOut, ServicesError> {
    let url = base_url.trim_end_matches('/');
    let action = if start { "start" } else { "stop" };
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    client
        .post(format!("{url}/services/{service}/{action}"))
        .json(&serde_json::json!({"operation_id": operation_id}))
        .send()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .error_for_status()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .json::<ActuateOut>()
        .map_err(|err| ServicesError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

/// Novo id de operação para um clique (cada clique é uma intenção nova).
#[must_use]
pub fn fresh_operation_id(service: &str) -> String {
    format!(
        "gui-{service}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    )
}

/// Mensagem do worker de serviços.
mod services_panel;
mod services_poll;

pub(crate) use services_panel::ServicesMsg;
pub use services_panel::{AuditEntry, ServicesPanel};
