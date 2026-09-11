//! Estado de apresentação do Studio: somente leitura sobre o catálogo real.
//!
//! `AppState` nunca inventa linhas — reflete o [`Snapshot`] do [`Catalog`];
//! vazio até receber um snapshot de verdade.

use studio_core::Snapshot;

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
}

impl AppState {
    /// Estado inicial honesto: sem catálogo carregado, sem linhas.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            status: String::from("nenhum catálogo carregado"),
        }
    }

    /// Substitui as linhas pelo conteúdo do snapshot; nunca acumula.
    pub fn refresh_from(&mut self, snapshot: &Snapshot) {
        let mut rows: Vec<ItemRow> = snapshot
            .entries
            .iter()
            .map(|entry| ItemRow {
                id: entry.id.clone(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_core::Catalog;

    #[test]
    fn empty_when_no_catalog_loaded() {
        let state = AppState::new();

        assert!(state.rows().is_empty());
        assert_eq!(state.status(), "nenhum catálogo carregado");
    }

    #[test]
    fn refresh_reflects_real_catalog_snapshot() {
        let mut catalog = Catalog::new();
        catalog.publish("alpha", "1");
        catalog.publish("beta", "2");
        let snapshot = catalog.snapshot();
        let mut state = AppState::new();

        state.refresh_from(&snapshot);

        let rows = state.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "alpha");
        assert_eq!(rows[0].value, "1");
        assert_eq!(rows[0].revision, 1);
        assert_eq!(rows[1].id, "beta");
        assert_eq!(state.status(), "2 item(ns) no catálogo");
    }

    #[test]
    fn refresh_replaces_instead_of_accumulating() {
        let mut catalog = Catalog::new();
        catalog.publish("alpha", "1");
        let mut state = AppState::new();
        state.refresh_from(&catalog.snapshot());
        catalog.publish("beta", "2");

        state.refresh_from(&catalog.snapshot());

        let ids: Vec<&str> = state.rows().iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }
}
