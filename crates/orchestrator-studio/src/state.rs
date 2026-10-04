//! Estado de apresentação do Studio: somente leitura sobre o catálogo real.
//!
//! `AppState` nunca inventa linhas — reflete o [`Snapshot`] do [`Catalog`];
//! vazio até receber um snapshot de verdade. A leitura do nó (HTTP
//! bloqueante) roda em THREAD de trabalho (padrão `models.rs`: thread +
//! mpsc + `poll` por frame) — a thread de UI nunca bloqueia (REQ/T-820-19).

use std::sync::mpsc;

use studio_core::catalog::Snapshot;

use crate::origin::{fetch_node_summary, NodeSummary, OriginError};

/// Uma linha observável da tabela: id, valor atual e revisão por objeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    pub id: String,
    pub value: String,
    pub revision: u64,
}

/// Mensagem do worker de leitura do nó (url ecoada para o status).
struct NodeMsg {
    url: String,
    result: Result<NodeSummary, OriginError>,
}

/// Estado da janela principal: linhas da tabela mais mensagem de status.
#[derive(Debug, Default)]
pub struct AppState {
    rows: Vec<ItemRow>,
    status: String,
    node: Option<NodeSummary>,
    busy: bool,
    receiver: Option<mpsc::Receiver<NodeMsg>>,
}

impl AppState {
    /// Estado inicial honesto: sem catálogo carregado, sem linhas.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            status: String::from("nenhum catálogo carregado"),
            node: None,
            busy: false,
            receiver: None,
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

    /// `true` enquanto a leitura do nó roda em background (`poll` drena).
    #[must_use]
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Busca o resumo no nó em THREAD de trabalho (REQ/T-820-19) e atualiza
    /// o status via `poll`; falha preserva o estado anterior e registra o
    /// motivo (nunca inventa linhas). Clique durante `busy` é ignorado.
    pub fn refresh_from_node(&mut self, base_url: &str) {
        if self.busy {
            return;
        }
        let url = base_url.trim_end_matches('/').to_string();
        let status_url = url.clone();
        let (tx, rx) = mpsc::channel();
        let worker_url = url.clone();
        std::thread::spawn(move || {
            let result = fetch_node_summary(&worker_url);
            let _ = tx.send(NodeMsg { url, result });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.status = format!("conectando ao nó {status_url}…");
    }

    /// Drena o worker da leitura do nó; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(NodeMsg { url, result }) = rx.try_recv() {
                match result {
                    Ok(summary) => {
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
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
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
