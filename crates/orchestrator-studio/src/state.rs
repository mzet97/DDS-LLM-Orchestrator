//! Estado de apresentação do Studio: somente leitura sobre o catálogo real.
//!
//! `AppState` nunca inventa linhas — reflete o [`Snapshot`] do [`Catalog`];
//! vazio até receber um snapshot de verdade.

use studio_core::catalog::Snapshot;

use crate::origin::{fetch_node_summary, NodeSummary, OriginError};

/// Uma linha observável da tabela: id, valor atual e revisão por objeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    pub id: String,
    pub value: String,
    pub revision: u64,
}

/// Estado da janela principal: linhas da tabela mais mensagem de status.
#[derive(Debug, Default)]
pub struct AppState {
    rows: Vec<ItemRow>,
    status: String,
    node: Option<NodeSummary>,
}

impl AppState {
    /// Estado inicial honesto: sem catálogo carregado, sem linhas.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            status: String::from("nenhum catálogo carregado"),
            node: None,
        }
    }

    /// Substitui as linhas pelo conteúdo do snapshot; nunca acumula.
    pub fn refresh_from(&mut self, snapshot: &Snapshot) {
        let mut rows: Vec<ItemRow> = snapshot
            .items
            .iter()
            .map(|entry| ItemRow {
                id: entry.id.0.clone(),
                value: entry.value.clone(),
                revision: entry.revision.0,
            })
            .collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        self.status = format!("{} item(ns) no catálogo", rows.len());
        self.rows = rows;
    }

    /// Linhas atuais, ordenadas por id.
    #[must_use]
    pub fn rows(&self) -> &[ItemRow] {
        &self.rows
    }

    /// Mensagem de status para a barra inferior.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Resumo vivo do nó (versão + operações), quando conectado.
    #[must_use]
    pub fn node(&self) -> Option<&NodeSummary> {
        self.node.as_ref()
    }

    /// Busca o resumo no nó e atualiza o status; falha preserva o estado
    /// anterior e registra o motivo (nunca inventa linhas).
    pub fn refresh_from_node(&mut self, base_url: &str) {
        match fetch_node_summary(base_url) {
            Ok(summary) => {
                let url = base_url.trim_end_matches('/');
                self.status = format!(
                    "nó {url} · protocolo {}.{} · {} operação(ões)",
                    summary.version.major,
                    summary.version.minor,
                    summary.operations.len()
                );
                self.node = Some(summary);
            }
            Err(err @ OriginError::Incompatible { .. }) => {
                self.status = format!("origem bloqueada: {err}");
                self.node = None;
            }
            Err(err @ OriginError::Unreachable { .. }) => {
                self.status = format!("nó inalcançável: {err}");
                self.node = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_core::catalog::{Catalog, DefinitionId};
    use studio_core::revision::Generation;

    fn publish(catalog: &mut Catalog, id: &str, value: &str) {
        catalog
            .publish(
                DefinitionId(String::from(id)),
                None,
                String::from(value),
                Generation(0),
            )
            .expect("publicação de teste deve criar o item");
    }

    #[test]
    fn empty_when_no_catalog_loaded() {
        let state = AppState::new();

        assert!(state.rows().is_empty());
        assert_eq!(state.status(), "nenhum catálogo carregado");
    }

    #[test]
    fn refresh_reflects_real_catalog_snapshot() {
        let mut catalog = Catalog::new();
        publish(&mut catalog, "alpha", "1");
        publish(&mut catalog, "beta", "2");
        let snapshot = catalog.snapshot();
        let mut state = AppState::new();

        state.refresh_from(&snapshot);

        let rows = state.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "alpha");
        assert_eq!(rows[0].value, "1");
        assert_eq!(rows[0].revision, 0);
        assert_eq!(rows[1].id, "beta");
        assert_eq!(state.status(), "2 item(ns) no catálogo");
    }

    #[test]
    fn refresh_replaces_instead_of_accumulating() {
        let mut catalog = Catalog::new();
        publish(&mut catalog, "alpha", "1");
        let mut state = AppState::new();
        state.refresh_from(&catalog.snapshot());
        publish(&mut catalog, "beta", "2");

        state.refresh_from(&catalog.snapshot());

        let ids: Vec<&str> = state.rows().iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }
}
