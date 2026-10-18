//! Parte de `services` (extraída para manter os arquivos revisáveis).

use super::*;

impl ServicesPanel {
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
                            self.audit_push(
                                String::from("GET /services (Ler plano)"),
                                format!("ok · {} unidade(s)", list.len()),
                            );
                        }
                        self.list = list;
                        self.error.clear();
                        self.last_sync = Some(std::time::Instant::now());
                        self.last_sync_ms = Some(crate::machines::now_unix_ns() / 1_000_000);
                        self.last_sync_ms = Some(crate::machines::now_unix_ns() / 1_000_000);
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
                        Ok(out) => self.audit_push(
                            format!("POST /services/{service}/stop"),
                            format!("ok · acted={} active={}", out.acted, out.active),
                        ),
                        Err(err) => {
                            self.error = err.clone();
                            self.audit_push(format!("POST /services/{service}/stop"), err);
                        }
                    }
                    if let Some(start) = start {
                        match start {
                            Ok(out) => {
                                self.error.clear();
                                self.audit_push(
                                    format!("POST /services/{service}/start"),
                                    format!("ok · acted={} active={}", out.acted, out.active),
                                );
                                self.apply_list(then_list);
                            }
                            Err(err) => {
                                self.error = err.clone();
                                self.audit_push(format!("POST /services/{service}/start"), err);
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

    /// Registra na auditoria com carimbo de relógio (tela 3.8).
    fn audit_push(&mut self, action: String, outcome: String) {
        self.audit.push(AuditEntry {
            action,
            outcome,
            ts_ms: crate::machines::now_unix_ns() / 1_000_000,
        });
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
                self.audit_push(
                    String::from(route),
                    format!(
                        "{} · acted={} active={}",
                        if out.acted {
                            "aplicada"
                        } else {
                            "já convergido"
                        },
                        out.acted,
                        out.active
                    ),
                );
                self.apply_list(then_list);
                if !out.acted {
                    self.error = format!("{} já estava convergido", out.service);
                }
            }
            Err(err) => {
                self.error = err.clone();
                self.audit_push(String::from(route), err);
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
                    self.last_sync_ms = Some(crate::machines::now_unix_ns() / 1_000_000);
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
