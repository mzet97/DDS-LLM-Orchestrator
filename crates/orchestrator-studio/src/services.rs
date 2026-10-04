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
enum ServicesMsg {
    /// Resposta de `Ler plano` (`GET /services`).
    Listed(Result<Vec<ServiceStatus>, String>),
    /// Resposta de atuação + releitura do plano (mesma sequência do
    /// antigo `actuate_row`: atua, depois relê).
    Actuated {
        outcome: Result<ActuateOut, String>,
        then_list: Option<Result<Vec<ServiceStatus>, String>>,
    },
}

/// Estado do painel de serviços.
#[derive(Debug)]
pub struct ServicesPanel {
    pub url: String,
    pub list: Vec<ServiceStatus>,
    pub error: String,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    receiver: Option<mpsc::Receiver<ServicesMsg>>,
}

impl ServicesPanel {
    /// Padrão honesto: nó local.
    #[must_use]
    pub fn new() -> Self {
        Self {
            url: String::from("http://127.0.0.1:4317"),
            list: Vec::new(),
            error: String::new(),
            busy: false,
            receiver: None,
        }
    }

    /// Recarrega o plano em background; erro preserva a lista e registra o
    /// motivo. Clique durante `busy` é ignorado.
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ServicesMsg::Listed(
                list_services(&url).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.error = "lendo plano…".into();
    }

    /// Efetiva start/stop na linha em BACKGROUND e relê o plano em seguida
    /// (mesma sequência, fora da thread de UI — REQ/T-820-19). Clique
    /// durante `busy` é ignorado; `poll` aplica o desfecho no painel.
    pub fn actuate_row(&mut self, service: &str, start: bool) {
        if self.busy {
            return;
        }
        let id = fresh_operation_id(service);
        let url = self.url.clone();
        let service_owned = service.to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = actuate(&url, &service_owned, start, &id).map_err(|err| err.to_string());
            let then_list = match outcome {
                Ok(_) => Some(list_services(&url).map_err(|err| err.to_string())),
                Err(_) => None,
            };
            let _ = tx.send(ServicesMsg::Actuated { outcome, then_list });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.error = "atuando…".into();
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ServicesMsg::Listed(result) => match result {
                        Ok(list) => {
                            self.list = list;
                            self.error.clear();
                        }
                        Err(err) => {
                            self.error = err;
                        }
                    },
                    ServicesMsg::Actuated { outcome, then_list } => match outcome {
                        Ok(out) => {
                            self.error.clear();
                            if let Some(listed) = then_list {
                                match listed {
                                    Ok(list) => {
                                        self.list = list;
                                        self.error.clear();
                                    }
                                    Err(err) => {
                                        self.error = err;
                                    }
                                }
                            }
                            if !out.acted {
                                self.error = format!("{} já estava convergido", out.service);
                            }
                        }
                        Err(err) => {
                            self.error = err;
                        }
                    },
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
    }
}

impl Default for ServicesPanel {
    fn default() -> Self {
        Self::new()
    }
}
