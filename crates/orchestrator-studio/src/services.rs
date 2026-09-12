//! Serviços do nó no Studio: plano legível (`GET /services`).
//!
//! Reutiliza `ServiceStatus` do `studio-node`: pretendido × efetivo.
//! Divergência é diff, nunca efeito — este painel não altera o host.

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

/// Estado do painel de serviços.
#[derive(Debug, Clone)]
pub struct ServicesPanel {
    pub url: String,
    pub list: Vec<ServiceStatus>,
    pub error: String,
}

impl ServicesPanel {
    /// Padrão honesto: nó local.
    #[must_use]
    pub fn new() -> Self {
        Self {
            url: String::from("http://127.0.0.1:4317"),
            list: Vec::new(),
            error: String::new(),
        }
    }

    /// Recarrega o plano; erro preserva a lista e registra o motivo.
    pub fn refresh(&mut self) {
        match list_services(&self.url.clone()) {
            Ok(list) => {
                self.list = list;
                self.error.clear();
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }

    /// Efetiva start/stop na linha e relê o plano em seguida.
    pub fn actuate_row(&mut self, service: &str, start: bool) {
        let id = fresh_operation_id(service);
        match actuate(&self.url.clone(), service, start, &id) {
            Ok(out) => {
                self.error.clear();
                self.refresh();
                if !out.acted {
                    self.error = format!("{} já estava convergido", out.service);
                }
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }
}

impl Default for ServicesPanel {
    fn default() -> Self {
        Self::new()
    }
}
