//! Modo protegido (T-890-08, G-38/65): ações com efeito real em máquinas
//! remotas (actuar unidades systemd, executar workflows) só disparam com o
//! modo ARMADO — e, mesmo armado, exigem confirmação explícita por ação.
//!
//! Desarmado (default ao abrir a GUI), toda ação protegida é RECUSADA na
//! camada de UI com recusa tipada — nada trafega. **Nota de validação:** os
//! textos dos gates G-38/65 vivem no SDD mestre; esta implementação segue a
//! evidência do repositório ("modo protegido/benchmark" em REPORT.md 800)
//! e fica marcada para validação contra o SDD na revisão final.

/// Ação protegida pendente de confirmação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub description: String,
}

/// Resultado de [`ProtectedGuard::request`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectedOutcome {
    /// Armado: a ação requer confirmação explícita (`confirm_pending`).
    NeedsConfirmation(String),
    /// Desarmado: recusado na camada de UI — nada trafegou.
    Refused(String),
}

/// Guarda compartilhado da GUI (um por app). Padrão honesto (Default
/// derivado): DESARMADO — abrir a GUI nunca habilita efeitos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtectedGuard {
    pub armed: bool,
    pub pending: Option<PendingAction>,
    /// Última recusa (para exibição na UI; limpa ao armar).
    pub last_refusal: Option<String>,
}

impl ProtectedGuard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Liga/desliga o modo; desarmar cancela pendência e recusa.
    pub fn set_armed(&mut self, armed: bool) {
        self.armed = armed;
        if armed {
            self.last_refusal = None;
        } else {
            self.pending = None;
            self.last_refusal = Some(String::from(
                "modo protegido DESARMADO — ações com efeito real estão bloqueadas",
            ));
        }
    }

    /// Requisita uma ação protegida. Desarmado → [`ProtectedOutcome::Refused`]
    /// (recusa registrada, NADA executa); armado → guarda como pendente e
    /// pede confirmação.
    pub fn request(&mut self, description: impl Into<String>) -> ProtectedOutcome {
        let description = description.into();
        if !self.armed {
            let refusal = format!(
                "modo protegido desarmado: '{description}' NÃO executada — arme o modo na barra lateral"
            );
            self.last_refusal = Some(refusal.clone());
            return ProtectedOutcome::Refused(refusal);
        }
        self.pending = Some(PendingAction {
            description: description.clone(),
        });
        ProtectedOutcome::NeedsConfirmation(description)
    }

    /// Confirma a pendência: devolve a descrição para o chamador EXECUTAR.
    pub fn confirm_pending(&mut self) -> Option<String> {
        let pending = self.pending.take()?;
        Some(pending.description)
    }

    /// Cancela a pendência (sem executar nada).
    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.armed {
            "protegido: LIGADO"
        } else {
            "protegido: DESARMADO"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disarmed_by_default_and_requests_are_refused_without_side_effects() {
        let mut guard = ProtectedGuard::new();
        assert!(!guard.armed, "default DESARMADO");
        match guard.request("stop dds-agent") {
            ProtectedOutcome::Refused(refusal) => {
                assert!(refusal.contains("NÃO executada"));
            }
            other => panic!("deveria recusar, veio {other:?}"),
        }
        assert!(guard.pending.is_none(), "nada pendente ao recusar");
        assert_eq!(
            guard.last_refusal.as_deref(),
            Some("modo protegido desarmado: 'stop dds-agent' NÃO executada — arme o modo na barra lateral")
        );
    }

    #[test]
    fn armed_requires_explicit_confirmation_and_confirm_returns_description() {
        let mut guard = ProtectedGuard::new();
        guard.set_armed(true);
        match guard.request("executar workflow A→B→C") {
            ProtectedOutcome::NeedsConfirmation(desc) => {
                assert_eq!(desc, "executar workflow A→B→C");
            }
            other => panic!("armado deveria pedir confirmação, veio {other:?}"),
        }
        assert!(guard.pending.is_some());
        assert_eq!(
            guard.confirm_pending(),
            Some(String::from("executar workflow A→B→C"))
        );
        assert!(guard.pending.is_none(), "confirmação consome a pendência");
        assert_eq!(guard.confirm_pending(), None, "sem pendência nova");
    }

    #[test]
    fn cancel_and_disarm_clear_pending_state() {
        let mut guard = ProtectedGuard::new();
        guard.set_armed(true);
        let _ = guard.request("remover máquina");
        guard.cancel_pending();
        assert!(guard.pending.is_none());
        assert_eq!(guard.confirm_pending(), None);

        let _ = guard.request("remover máquina");
        guard.set_armed(false);
        assert!(guard.pending.is_none(), "desarmar cancela pendência");
    }
}
