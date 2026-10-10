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

use crate::origin::{fetch_node_summary_with_token, NodeSummary, OriginError};

/// Mensagem do worker de leitura do nó (url ecoada para o status).
struct NodeMsg {
    url: String,
    result: Result<NodeSummary, OriginError>,
    /// RTT medido do probe (duração integral da chamada, ms).
    rtt_ms: u64,
}

/// Estado da janela principal: resumo do nó mais mensagem de status.
#[derive(Debug, Default)]
pub struct AppState {
    status: String,
    node: Option<NodeSummary>,
    busy: bool,
    /// RTT do último probe concluído (PRD 3.2 — medição real, não estimada).
    rtt_ms: Option<u64>,
    /// EMA do RTT (α=0.3) para suavizar jitter sem esconder passos.
    rtt_ema_ms: Option<f64>,
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
            rtt_ms: None,
            rtt_ema_ms: None,
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
    /// Sem token — mantida para o modo localhost (T-840-03a).
    pub fn refresh_from_node(&mut self, base_url: &str) {
        self.refresh_from_node_with_token(base_url, None);
    }

    /// Como [`refresh_from_node`](Self::refresh_from_node), com token
    /// opcional (T-840-03a; nó conforme T-840-01). O token vive só na
    /// memória da GUI: é passado ao worker por valor e nunca persistido.
    pub fn refresh_from_node_with_token(&mut self, base_url: &str, token: Option<&str>) {
        if self.busy {
            return;
        }
        let url = base_url.trim_end_matches('/').to_string();
        let status_url = url.clone();
        let token = token.map(String::from);
        let (tx, rx) = mpsc::channel();
        let worker_url = url.clone();
        std::thread::spawn(move || {
            // RTT real do probe (PRD 3.2): duração integral da chamada —
            // conexão + requisição + resposta, medida no cliente.
            let started = std::time::Instant::now();
            let result = fetch_node_summary_with_token(&worker_url, token.as_deref());
            let rtt_ms = started.elapsed().as_millis() as u64;
            let _ = tx.send(NodeMsg {
                url,
                result,
                rtt_ms,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.status = format!("conectando ao nó {status_url}…");
    }

    /// RTT do último probe concluído (ms) — `None` sem probe concluído.
    #[must_use]
    pub fn rtt_ms(&self) -> Option<u64> {
        self.rtt_ms
    }

    /// EMA do RTT (ms, α=0.3) — suaviza o jitter dos probes.
    #[must_use]
    pub fn rtt_ema_ms(&self) -> Option<f64> {
        self.rtt_ema_ms
    }

    /// Drena o worker da leitura do nó; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(NodeMsg {
                url,
                result,
                rtt_ms,
            }) = rx.try_recv()
            {
                // EMA do RTT (α=0.3) independentemente do desfecho — um 401
                // ou refused também tem RTT real.
                self.rtt_ema_ms = Some(match self.rtt_ema_ms {
                    Some(ema) => ema * 0.7 + (rtt_ms as f64) * 0.3,
                    None => rtt_ms as f64,
                });
                match result {
                    Ok(summary) => {
                        self.rtt_ms = Some(rtt_ms);
                        self.status = format!(
                            "nó {url} · protocolo {}.{} · {} operação(ões) · rtt {rtt_ms} ms",
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
                    Err(err @ OriginError::Unauthorized) => {
                        self.status = format!("acesso ao nó negado: {err}");
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
