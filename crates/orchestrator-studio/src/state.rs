//! Estado de apresentação do Studio: leitura viva do nó studio-node.
//!
//! `AppState` nunca inventa dados — só reflete o resumo real do nó
//! ([`NodeSummary`]) e mensagens de status honestas. A leitura do nó (HTTP
//! bloqueante) roda em THREAD de trabalho (padrão `models.rs`: thread +
//! mpsc + `poll` por frame) — a thread de UI nunca bloqueia (REQ/T-820-19).
//!
//! O painel local de "Catálogo" foi removido (T-830-04): o `Catalog` em
//! memória do Studio nunca tinha linhas na prática, e a leitura real do
//! catálogo compartilhado vive em `catalog_remote::SharedCatalog` (mesma
//! autoridade do nó).

use std::sync::mpsc;

use crate::origin::{fetch_node_summary, NodeSummary, OriginError};

/// Mensagem do worker de leitura do nó (url ecoada para o status).
struct NodeMsg {
    url: String,
    result: Result<NodeSummary, OriginError>,
}

/// Estado da janela principal: resumo do nó mais mensagem de status.
#[derive(Debug, Default)]
pub struct AppState {
    status: String,
    node: Option<NodeSummary>,
    busy: bool,
    receiver: Option<mpsc::Receiver<NodeMsg>>,
}

impl AppState {
    /// Estado inicial honesto: sem leitura do nó ainda.
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: String::from("sem leitura do nó"),
            node: None,
            busy: false,
            receiver: None,
        }
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

    #[test]
    fn initial_state_is_honest_without_node() {
        let state = AppState::new();
        assert!(state.node().is_none());
        assert!(!state.busy());
        assert_eq!(state.status(), "sem leitura do nó");
    }
}
