//! Atuador de serviços próprios: efetiva o pretendido no gerenciador.
//!
//! Separado da `Probe` (leitura) de propósito: ler é sempre seguro, agir
//! exige operação idempotente registrada — o servidor só age dentro de
//! `POST /services/:name/start|stop` com `operation_id`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Falha do atuador (fronteira nó ↔ gerenciador).
#[derive(Debug, Error)]
pub enum ActuatorError {
    /// O gerenciador recusou ou falhou.
    #[error("atuador falhou em {service}: {detail}")]
    Failed { service: String, detail: String },
}

/// Executa start/stop em serviço próprio.
pub trait Actuator: Send + Sync {
    /// Leva o serviço ao estado pedido; idempotente no gerenciador.
    fn set_active(&self, service: &str, active: bool) -> Result<(), ActuatorError>;
}

/// Atuador real via `systemctl --user start|stop`.
#[derive(Debug, Default)]
pub struct SystemdActuator;

impl Actuator for SystemdActuator {
    fn set_active(&self, service: &str, active: bool) -> Result<(), ActuatorError> {
        let action = if active { "start" } else { "stop" };
        let status = std::process::Command::new("systemctl")
            .args(["--user", action, service])
            .status()
            .map_err(|err| ActuatorError::Failed {
                service: String::from(service),
                detail: err.to_string(),
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(ActuatorError::Failed {
                service: String::from(service),
                detail: format!("systemctl {action} saiu {status}"),
            })
        }
    }
}

/// Atuador de teste: registra chamadas e guarda estados em memória.
#[derive(Debug, Default)]
pub struct FakeActuator {
    states: Mutex<HashMap<String, bool>>,
    calls: Mutex<Vec<(String, bool)>>,
}

impl FakeActuator {
    /// Atuador vazio empacotado para o servidor.
    #[must_use]
    pub fn arc() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Chamadas efetuadas, na ordem.
    #[must_use]
    pub fn calls(&self) -> Vec<(String, bool)> {
        self.calls.lock().expect("acessivel").clone()
    }
}

impl Actuator for FakeActuator {
    fn set_active(&self, service: &str, active: bool) -> Result<(), ActuatorError> {
        self.states
            .lock()
            .expect("acessivel")
            .insert(String::from(service), active);
        self.calls
            .lock()
            .expect("acessivel")
            .push((String::from(service), active));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_records_calls_in_order() {
        let actuator = FakeActuator::arc();

        actuator.set_active("a", true).expect("ok");
        actuator.set_active("a", false).expect("ok");

        assert_eq!(
            actuator.calls(),
            vec![(String::from("a"), true), (String::from("a"), false),]
        );
    }
}
