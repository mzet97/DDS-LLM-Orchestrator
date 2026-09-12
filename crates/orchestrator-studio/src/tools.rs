//! Ferramentas e gateways (T06/UI-3.2): definição, restrições e painel
//! de teste. Tipo sem executor mostra "Requer implementação de
//! executor"; nunca shell/MCP improvisado. Associar ≠ loop de tool-use.

use serde::{Deserialize, Serialize};

/// Como a ferramenta é implementada (tipos suportados executam; o resto
/// é capacidade indisponível explícita).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolImpl {
    Http,
    Local,
    Unsupported,
}

impl ToolImpl {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Local => "local",
            Self::Unsupported => "sem executor",
        }
    }
}

/// Definição de ferramenta (rascunho local).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub implementation: ToolImpl,
    pub gateway: String,
    pub machine: String,
    pub declared_restriction: String,
    pub observed_restriction: Option<String>,
}

impl ToolDefinition {
    #[must_use]
    pub fn draft(id: &str) -> Self {
        Self {
            id: String::from(id),
            name: String::new(),
            description: String::new(),
            implementation: ToolImpl::Unsupported,
            gateway: String::new(),
            machine: String::new(),
            declared_restriction: String::new(),
            observed_restriction: None,
        }
    }

    /// Linha de restrição: declarada × efetivamente observada.
    #[must_use]
    pub fn restriction_line(&self) -> String {
        match &self.observed_restriction {
            Some(observed) => format!(
                "declarada: {}; observada: {observed}",
                self.declared_restriction
            ),
            None => format!(
                "declarada: {}; ainda não observada",
                self.declared_restriction
            ),
        }
    }
}

/// Resultado do painel de teste (entrada, destino, resultado/erro,
/// duração e ID de operação — sem efeito automático ao abrir o editor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTestResult {
    pub input: String,
    pub destination: String,
    pub output: String,
    pub duration_ms: u64,
    pub operation_id: String,
}

/// Estado do painel de ferramentas.
#[derive(Debug, Clone, Default)]
pub struct ToolsPanel {
    pub tools: Vec<ToolDefinition>,
    pub selected: Option<String>,
    pub last_test: Option<ToolTestResult>,
    next_id: u64,
}

impl ToolsPanel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cria rascunho com identidade nova.
    pub fn create_draft(&mut self) -> &ToolDefinition {
        self.next_id += 1;
        let id = format!("tool-{}", self.next_id);
        self.tools.push(ToolDefinition::draft(&id));
        self.tools.last().expect("rascunho criado")
    }

    /// Seleciona por ID; inexistente limpa.
    pub fn select(&mut self, id: &str) {
        if self.tools.iter().any(|item| item.id == id) {
            self.selected = Some(String::from(id));
        } else {
            self.selected = None;
        }
    }

    #[must_use]
    pub fn selected(&self) -> Option<&ToolDefinition> {
        let id = self.selected.as_ref()?;
        self.tools.iter().find(|item| &item.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_impl_reports_restriction_gap() {
        let mut panel = ToolsPanel::new();
        let draft = panel.create_draft();
        assert_eq!(draft.implementation, ToolImpl::Unsupported);
        assert!(draft.restriction_line().contains("ainda não observada"));
    }

    #[test]
    fn observed_restriction_shown_beside_declared() {
        let mut definition = ToolDefinition::draft("tool-1");
        definition.declared_restriction = String::from("somente leitura");
        definition.observed_restriction = Some(String::from("escrita negada em teste"));
        let line = definition.restriction_line();
        assert!(line.contains("somente leitura"));
        assert!(line.contains("escrita negada em teste"));
    }

    #[test]
    fn select_unknown_clears_selection() {
        let mut panel = ToolsPanel::new();
        panel.create_draft();
        panel.select("tool-9");
        assert_eq!(panel.selected(), None);
    }
}
