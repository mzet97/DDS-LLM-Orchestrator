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
        route: String,
        outcome: Result<ActuateOut, String>,
        then_list: Option<Result<Vec<ServiceStatus>, String>>,
    },
    /// Resposta do reinício (stop → start) + releitura do plano.
    Restarted {
        service: String,
        stop: Result<ActuateOut, String>,
        start: Option<Result<ActuateOut, String>>,
        then_list: Option<Result<Vec<ServiceStatus>, String>>,
    },
}

/// Entrada do log de auditoria de operações systemd (tela 3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Ação aplicada (ex.: "POST /services/llama-server/start").
    pub action: String,
    /// Desfecho legível (ok com `acted/active` ou o motivo da falha).
    pub outcome: String,
}

/// Estado do painel de serviços.
#[derive(Debug)]
pub struct ServicesPanel {
    /// Ação aceita pelo modo protegido e aguardando confirmação (T-890-08).
    pub pending_action: Option<(String, bool)>,
    /// Reinício pendente de confirmação (T-890-UX: ↺ = stop+start).
    pub pending_restart: Option<String>,
    pub url: String,
    pub list: Vec<ServiceStatus>,
    pub error: String,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    /// Instante do último plano lido com sucesso (tela 3.8).
    pub last_sync: Option<std::time::Instant>,
    /// Log de auditoria das atuações aplicadas pela GUI (tela 3.8), mais
    /// recente por último.
    pub audit: Vec<AuditEntry>,
    /// `true` quando a leitura em curso foi um clique em "Ler plano" (a
    /// auditoria registra GETs manuais, não a auto-carga — PRD 3.8).
    pub audit_manual_list: bool,
    receiver: Option<mpsc::Receiver<ServicesMsg>>,
}

impl ServicesPanel {
    /// Padrão honesto: nó local.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_action: None,
            pending_restart: None,
            url: String::from("http://127.0.0.1:4317"),
            list: Vec::new(),
            error: String::new(),
            busy: false,
            last_sync: None,
            audit: Vec::new(),
            audit_manual_list: false,
            receiver: None,
        }
    }

    /// Painel apontando para um nó específico (testes de UI — T-890-08).
    #[must_use]
    pub fn with_url(url: String) -> Self {
        Self { url, ..Self::new() }
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
    /// (mesma sequência, fora da thread de UI — REQ/T-820-19). Clique durante
    /// `busy` é ignorado; `poll` aplica o desfecho no painel.
    pub fn actuate_row(&mut self, service: &str, start: bool) {
        if self.busy {
            return;
        }
        let id = fresh_operation_id(service);
        let url = self.url.clone();
        let service_owned = service.to_string();
        let (tx, rx) = mpsc::channel();
        let route = format!(
            "POST /services/{service_owned}/{}",
            if start { "start" } else { "stop" }
        );
        std::thread::spawn(move || {
            let outcome = actuate(&url, &service_owned, start, &id).map_err(|err| err.to_string());
            let then_list = match outcome {
                Ok(_) => Some(list_services(&url).map_err(|err| err.to_string())),
                Err(_) => None,
            };
            let _ = tx.send(ServicesMsg::Actuated {
                route,
                outcome,
                then_list,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.error = "atuando…".into();
    }

    /// Reinicia a unidade em BACKGROUND: stop → start → relê o plano
    /// (↺ da tela 3.8; mesma proteção do modo protegido). Clique durante
    /// `busy` é ignorado.
    pub fn restart_row(&mut self, service: &str) {
        if self.busy {
            return;
        }
        let stop_id = fresh_operation_id(service);
        let start_id = fresh_operation_id(service);
        let url = self.url.clone();
        let service_owned = service.to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let stop =
                actuate(&url, &service_owned, false, &stop_id).map_err(|err| err.to_string());
            let start = if stop.is_ok() {
                Some(actuate(&url, &service_owned, true, &start_id).map_err(|err| err.to_string()))
            } else {
                None
            };
            let then_list = if start.as_ref().is_some_and(Result::is_ok) {
                Some(list_services(&url).map_err(|err| err.to_string()))
            } else {
                None
            };
            let _ = tx.send(ServicesMsg::Restarted {
                service: service_owned,
                stop,
                start,
                then_list,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.error = "reiniciando (stop→start)…".into();
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        // Drena primeiro para uma fila local (o empréstimo de `receiver`
        // não pode coexistir com os métodos `&mut self` que aplicam).
        let mut drained: Vec<ServicesMsg> = Vec::new();
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                drained.push(msg);
            }
        }
        let finished = !drained.is_empty();
        for msg in drained {
            match msg {
                ServicesMsg::Listed(result) => match result {
                    Ok(list) => {
                        // PRD 3.8: leitura manual também entra na auditoria
                        // (terminal GET+POST — só quando foi clique, não na
                        // auto-carga de 5 s).
                        if self.audit_manual_list {
                            self.audit_manual_list = false;
                            self.audit.push(AuditEntry {
                                action: String::from("GET /services (Ler plano)"),
                                outcome: format!("ok · {} unidade(s)", list.len()),
                            });
                        }
                        self.list = list;
                        self.error.clear();
                        self.last_sync = Some(std::time::Instant::now());
                    }
                    Err(err) => {
                        self.error = err;
                    }
                },
                ServicesMsg::Actuated {
                    route,
                    outcome,
                    then_list,
                } => {
                    self.apply_actuated(&route, outcome, then_list);
                }
                ServicesMsg::Restarted {
                    service,
                    stop,
                    start,
                    then_list,
                } => {
                    // Auditoria do reinício: duas entradas (stop e start).
                    match stop {
                        Ok(out) => self.audit.push(AuditEntry {
                            action: format!("POST /services/{service}/stop"),
                            outcome: format!("ok · acted={} active={}", out.acted, out.active),
                        }),
                        Err(err) => {
                            self.error = err.clone();
                            self.audit.push(AuditEntry {
                                action: format!("POST /services/{service}/stop"),
                                outcome: err,
                            });
                        }
                    }
                    if let Some(start) = start {
                        match start {
                            Ok(out) => {
                                self.error.clear();
                                self.audit.push(AuditEntry {
                                    action: format!("POST /services/{service}/start"),
                                    outcome: format!(
                                        "ok · acted={} active={}",
                                        out.acted, out.active
                                    ),
                                });
                                self.apply_list(then_list);
                            }
                            Err(err) => {
                                self.error = err.clone();
                                self.audit.push(AuditEntry {
                                    action: format!("POST /services/{service}/start"),
                                    outcome: err,
                                });
                            }
                        }
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
        // Auditoria é um log: mantém só as últimas 32 entradas.
        if self.audit.len() > 32 {
            let drop = self.audit.len() - 32;
            self.audit.drain(0..drop);
        }
    }

    /// Aplica o desfecho de uma atuação start/stop (+ relista o plano).
    fn apply_actuated(
        &mut self,
        route: &str,
        outcome: Result<ActuateOut, String>,
        then_list: Option<Result<Vec<ServiceStatus>, String>>,
    ) {
        match outcome {
            Ok(out) => {
                self.error.clear();
                self.audit.push(AuditEntry {
                    action: String::from(route),
                    outcome: format!(
                        "{} · acted={} active={}",
                        if out.acted {
                            "aplicada"
                        } else {
                            "já convergido"
                        },
                        out.acted,
                        out.active
                    ),
                });
                self.apply_list(then_list);
                if !out.acted {
                    self.error = format!("{} já estava convergido", out.service);
                }
            }
            Err(err) => {
                self.error = err.clone();
                self.audit.push(AuditEntry {
                    action: String::from(route),
                    outcome: err,
                });
            }
        }
    }

    /// Aplica a releitura do plano após atuação (quando existe).
    fn apply_list(&mut self, then_list: Option<Result<Vec<ServiceStatus>, String>>) {
        if let Some(listed) = then_list {
            match listed {
                Ok(list) => {
                    self.list = list;
                    self.error.clear();
                    self.last_sync = Some(std::time::Instant::now());
                }
                Err(err) => {
                    self.error = err;
                }
            }
        }
    }
}

impl Default for ServicesPanel {
    fn default() -> Self {
        Self::new()
    }
}
