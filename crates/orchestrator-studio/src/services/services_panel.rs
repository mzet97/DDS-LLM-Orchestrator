//! Parte de `services` (extraída para manter os arquivos revisáveis).

use super::*;

pub(crate) enum ServicesMsg {
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
    /// Instante (ms unix) do registro (coluna de relógio da 3.8).
    pub ts_ms: u64,
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
    /// Mesmo instante em ms unix (relógio absoluto da 3.8).
    pub last_sync_ms: Option<u64>,
    /// Log de auditoria das atuações aplicadas pela GUI (tela 3.8), mais
    /// recente por último.
    pub audit: Vec<AuditEntry>,
    /// `true` quando a leitura em curso foi um clique em "Ler plano" (a
    /// auditoria registra GETs manuais, não a auto-carga — PRD 3.8).
    pub audit_manual_list: bool,
    pub(crate) receiver: Option<mpsc::Receiver<ServicesMsg>>,
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
            last_sync_ms: None,
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
}
