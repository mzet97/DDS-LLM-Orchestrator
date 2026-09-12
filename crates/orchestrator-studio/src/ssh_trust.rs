//! Confiança de hosts para a futura bridge SSH (G-02, P3).
//!
//! Lógica pura, sem rede: guarda por projeto/host a impressão digital
//! (fingerprint) da host key apresentada pelo servidor. Primeira conexão
//! exige aprovação explícita do operador (TOFU com decisão registrada);
//! chave alterada BLOQUEIA até decisão explícita — nunca reconecta
//! silenciosamente. O transporte SSH real (G-04) continua pendente e usa
//! este cofre como autoridade de confiança.

use thiserror::Error;

/// Decisão registrada para um host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustDecision {
    /// Operador aprovou esta impressão digital.
    Approved { fingerprint: String },
}

/// Erros do cofre (decisões de confiança, sem I/O).
#[derive(Debug, Error)]
pub enum TrustError {
    /// Host novo: nada aprovado ainda — exige decisão explícita.
    #[error("host {host} desconhecido: aprove a impressao {fingerprint} antes de conectar")]
    Unknown { host: String, fingerprint: String },
    /// Chave alterada: bloqueia até decisão explícita.
    #[error("host {host} mudou a chave (era {known}, veio {presented}): reconexao bloqueada")]
    Changed {
        host: String,
        known: String,
        presented: String,
    },
}

/// Cofre de confiança: host → impressão aprovada.
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    entries: Vec<(String, TrustDecision)>,
}

impl TrustStore {
    /// Cofre vazio: nenhum host confiável.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Verifica a chave apresentada: aprovado passa, resto é erro tipado.
    pub fn check(&self, host: &str, fingerprint: &str) -> Result<(), TrustError> {
        match self.entries.iter().find(|(name, _)| name == host) {
            None => Err(TrustError::Unknown {
                host: String::from(host),
                fingerprint: String::from(fingerprint),
            }),
            Some((_, TrustDecision::Approved { fingerprint: known })) if known == fingerprint => {
                Ok(())
            }
            Some((_, TrustDecision::Approved { fingerprint: known })) => Err(TrustError::Changed {
                host: String::from(host),
                known: known.clone(),
                presented: String::from(fingerprint),
            }),
        }
    }

    /// Aprovação explícita do operador (primeira vez ou após bloqueio).
    /// Substitui o registro anterior sem apagar a história de decisão:
    /// quem chama deve auditar a troca.
    pub fn approve(&mut self, host: &str, fingerprint: &str) {
        self.entries.retain(|(name, _)| name != host);
        self.entries.push((
            String::from(host),
            TrustDecision::Approved {
                fingerprint: String::from(fingerprint),
            },
        ));
    }

    /// Hosts com confiança registrada.
    #[must_use]
    pub fn hosts(&self) -> Vec<&str> {
        self.entries.iter().map(|(name, _)| name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_host_requires_explicit_approval() {
        let store = TrustStore::new();

        let err = store
            .check("192.168.1.61", "SHA256:AAA")
            .expect_err("novo bloqueia");

        assert!(matches!(err, TrustError::Unknown { .. }));
    }

    #[test]
    fn approved_key_passes_same_key() {
        let mut store = TrustStore::new();
        store.approve("192.168.1.61", "SHA256:AAA");

        assert!(store.check("192.168.1.61", "SHA256:AAA").is_ok());
        assert_eq!(store.hosts(), vec!["192.168.1.61"]);
    }

    #[test]
    fn changed_key_blocks_until_explicit_reapproval() {
        let mut store = TrustStore::new();
        store.approve("192.168.1.61", "SHA256:AAA");

        let err = store
            .check("192.168.1.61", "SHA256:BBB")
            .expect_err("mudanca bloqueia");
        assert!(matches!(err, TrustError::Changed { .. }));
        // Sem aprovação nova, continua bloqueado (sem fallback silencioso).
        assert!(store.check("192.168.1.61", "SHA256:BBB").is_err());
        store.approve("192.168.1.61", "SHA256:BBB");
        assert!(store.check("192.168.1.61", "SHA256:BBB").is_ok());
    }
}
