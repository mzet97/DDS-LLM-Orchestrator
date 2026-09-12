//! Nós conhecidos do Studio: administrar N nós de uma GUI (P3-admin).
//!
//! Somente endereços já autorizados pelo operador: este registro guarda
//! `(apelido, URL)` digitados na GUI, nunca varre rede nem testa
//! credenciais. Cadastro SSH com host key (G-02/G-04) continua pendente e
//! fora deste módulo. Vazio por padrão: nenhum nó é presumido.

use thiserror::Error;

/// Entrada do registro: apelido local + URL administrativa do nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEntry {
    pub alias: String,
    pub url: String,
}

/// Erros do registro (validação local, sem efeito em rede).
#[derive(Debug, Error)]
pub enum NodesError {
    /// Apelido ou URL vazios, URL fora de http(s) ou duplicata.
    #[error("no invalido: {detail}")]
    Invalid { detail: String },
}

/// Registro de nós com seleção atual e rascunho do formulário.
#[derive(Debug, Clone)]
pub struct NodeRegistry {
    entries: Vec<NodeEntry>,
    selected: Option<usize>,
    pub draft_alias: String,
    pub draft_url: String,
    pub error: String,
}

impl NodeRegistry {
    /// Registro vazio honesto: nenhum nó conhecido até o operador digitar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: None,
            draft_alias: String::new(),
            draft_url: String::new(),
            error: String::new(),
        }
    }

    /// Entradas ordenadas de inserção.
    #[must_use]
    pub fn list(&self) -> &[NodeEntry] {
        &self.entries
    }

    /// Selecionado atual, quando houver.
    #[must_use]
    pub fn selected(&self) -> Option<&NodeEntry> {
        self.selected.and_then(|index| self.entries.get(index))
    }

    /// Adiciona após validar; duplicata de apelido ou URL é recusada.
    pub fn add(&mut self, alias: &str, url: &str) -> Result<(), NodesError> {
        let invalid = |detail: &str| NodesError::Invalid {
            detail: String::from(detail),
        };
        let alias = alias.trim();
        let url = url.trim().trim_end_matches('/');
        if alias.is_empty() || url.is_empty() {
            return Err(invalid("apelido e URL precisam ser preenchidos"));
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(invalid("URL precisa começar com http:// ou https://"));
        }
        if self.entries.iter().any(|entry| entry.alias == alias) {
            return Err(invalid("apelido já cadastrado"));
        }
        if self.entries.iter().any(|entry| entry.url == url) {
            return Err(invalid("URL já cadastrada em outro apelido"));
        }
        self.entries.push(NodeEntry {
            alias: String::from(alias),
            url: String::from(url),
        });
        if self.selected.is_none() {
            self.selected = Some(self.entries.len() - 1);
        }
        Ok(())
    }

    /// Remove por apelido; seleção cai para o primeiro restante ou some.
    pub fn remove(&mut self, alias: &str) {
        self.entries.retain(|entry| entry.alias != alias);
        self.selected = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Seleciona por apelido; desconhecido mantém a seleção anterior.
    pub fn select(&mut self, alias: &str) {
        if let Some(index) = self.entries.iter().position(|entry| entry.alias == alias) {
            self.selected = Some(index);
        }
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_with_nothing_selected() {
        let registry = NodeRegistry::new();

        assert!(registry.list().is_empty());
        assert!(registry.selected().is_none());
    }

    #[test]
    fn rejects_garbage_before_any_network() {
        let mut registry = NodeRegistry::new();

        assert!(registry.add("", "http://127.0.0.1:4317").is_err());
        assert!(registry.add("local", "127.0.0.1:4317").is_err());
        assert!(registry.list().is_empty());
    }

    #[test]
    fn first_entry_becomes_selected_and_duplicates_refused() {
        let mut registry = NodeRegistry::new();
        registry
            .add("local", "http://127.0.0.1:4317")
            .expect("vale");
        registry
            .add("gpu", "http://192.168.1.61:4317")
            .expect("vale");

        assert_eq!(registry.selected().expect("selecionado").alias, "local");
        assert!(registry.add("local", "http://192.168.1.62:4317").is_err());
        assert!(registry.add("orc", "http://192.168.1.61:4317").is_err());
        assert_eq!(registry.list().len(), 2);
    }

    #[test]
    fn select_and_remove_keep_valid_selection() {
        let mut registry = NodeRegistry::new();
        registry.add("a", "http://10.0.0.1:4317").expect("vale");
        registry.add("b", "http://10.0.0.2:4317").expect("vale");

        registry.select("b");
        assert_eq!(registry.selected().expect("b").alias, "b");
        registry.select("fantasma");
        assert_eq!(registry.selected().expect("ainda b").alias, "b");
        registry.remove("b");
        assert_eq!(registry.selected().expect("caiu para a").alias, "a");
        registry.remove("a");
        assert!(registry.selected().is_none());
    }
}
