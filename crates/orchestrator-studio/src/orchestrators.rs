//! Orquestradores (T09/UI-2.4): o monitor como supervisão de tarefas
//! e agentes — definição, host de destino, domínio, configuração
//! suportada, instâncias e evidências. Nunca apresentado como motor
//! que necessariamente executa o encadeamento.

use serde::{Deserialize, Serialize};

/// Definição do monitor (rascunho local até aplicação).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorDefinition {
    pub alias: String,
    pub host: String,
    pub domain: String,
    pub config: String,
    pub applied: bool,
}

impl MonitorDefinition {
    #[must_use]
    pub fn new(alias: &str) -> Self {
        Self {
            alias: String::from(alias),
            host: String::new(),
            domain: String::from("78"),
            config: String::from("Balanced"),
            applied: false,
        }
    }

    /// Papel exibido junto a todo resumo (rótulo auxiliar exigido).
    #[must_use]
    pub const fn role() -> &'static str {
        "Supervisão de tarefas e agentes"
    }
}

/// Estado do painel: definição em edição + contagens observadas.
#[derive(Debug, Clone, Default)]
pub struct OrchestratorPanel {
    pub definition: Option<MonitorDefinition>,
    pub draft_alias: String,
    pub observed_agents: usize,
    pub observed_operations: usize,
    pub last_evidence: String,
}

impl OrchestratorPanel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Prepara definição nova; aplicar é intenção registrada, não efeito.
    pub fn prepare(&mut self) {
        let alias = self.draft_alias.trim();
        if !alias.is_empty() {
            self.definition = Some(MonitorDefinition::new(alias));
            self.draft_alias.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_names_supervision_not_execution() {
        assert_eq!(MonitorDefinition::role(), "Supervisão de tarefas e agentes");
    }

    #[test]
    fn prepare_starts_unapplied_with_defaults() {
        let mut panel = OrchestratorPanel::new();
        panel.draft_alias = String::from("monitor-01");
        panel.prepare();
        let definition = panel.definition.expect("definição");
        assert_eq!(definition.domain, "78");
        assert!(!definition.applied);
    }

    #[test]
    fn empty_alias_prepares_nothing() {
        let mut panel = OrchestratorPanel::new();
        panel.draft_alias = String::from("   ");
        panel.prepare();
        assert_eq!(panel.definition, None);
    }
}
