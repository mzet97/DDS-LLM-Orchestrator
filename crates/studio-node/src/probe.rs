//! Sonda de estado real dos serviços do nó (base do plano/diff).
//!
//! `wanted` vem do log (último `SetService`); `active` vem do gerenciador.
//! Divergência entre os dois é o diff legível — sem executar efeitos.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Leitor do estado efetivo de um serviço (borda com o SO).
pub trait Probe: Send + Sync {
    /// `true` se o serviço está ativo agora.
    fn is_active(&self, service: &str) -> bool;
}

/// Sonda real via `systemctl --user is-active --quiet` (exit 0 = ativo).
#[derive(Debug, Default)]
pub struct SystemdProbe;

impl Probe for SystemdProbe {
    fn is_active(&self, service: &str) -> bool {
        std::process::Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", service])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

/// Sonda de teste com estados fixos.
#[derive(Debug, Default)]
pub struct FakeProbe {
    states: Mutex<HashMap<String, bool>>,
}

impl FakeProbe {
    /// Sonda com os estados dados (`serviço → ativo?`).
    #[must_use]
    pub fn with(states: &[(&str, bool)]) -> Arc<Self> {
        Arc::new(Self {
            states: Mutex::new(
                states
                    .iter()
                    .map(|(name, active)| (String::from(*name), *active))
                    .collect(),
            ),
        })
    }
}

impl Probe for FakeProbe {
    fn is_active(&self, service: &str) -> bool {
        self.states
            .lock()
            .expect("sonda acessivel")
            .get(service)
            .copied()
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_reports_configured_states() {
        let probe = FakeProbe::with(&[("a", true)]);

        assert!(probe.is_active("a"));
        assert!(!probe.is_active("b"));
    }

    #[test]
    fn systemd_probe_answers_without_panic() {
        let _ = SystemdProbe.is_active("studio-noded");
    }
}
