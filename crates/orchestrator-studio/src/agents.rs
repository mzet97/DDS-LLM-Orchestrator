//! Agentes no Studio: leitura viva do orquestrador (`GET /api/v1/agents`).
//!
//! Números crus do backend, sem semáforo inventado: saúde, slots e
//! contadores aparecem como o orquestrador anuncia.
//!
//! O HTTP bloqueante roda em THREAD de trabalho (padrão `models.rs`/
//! `catalog_remote.rs`: thread + mpsc + `poll` por frame — REQ/T-820-19,
//! agora também aqui: T-830-01). A thread de UI nunca bloqueia.

use std::sync::mpsc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Agente anunciado pelo orquestrador (subconjunto exibido na GUI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub model: String,
    pub specialization: String,
    pub hostname: String,
    pub health: u32,
    pub slots_busy: u32,
    pub slots_total: u32,
    pub completed_total: u64,
    pub failed_total: u64,
    pub ema_latency_ms: f32,
}

/// Erros da leitura de agentes (fronteira GUI ↔ orquestrador).
#[derive(Debug, Error)]
pub enum AgentsError {
    /// Orquestrador inalcançável ou fora do contrato.
    #[error("falha ao ler agentes em {url}: {detail}")]
    Unreachable { url: String, detail: String },
}

/// Mensagem do worker de agentes: lista crua ou motivo da falha
/// (`AgentsError` já serializado em texto pelo `to_string`).
type AgentsMsg = Result<Vec<AgentInfo>, String>;

/// Estado do painel de agentes: URL, última lista e último erro.
#[derive(Debug, Default)]
pub struct AgentsState {
    pub url: String,
    pub list: Vec<AgentInfo>,
    pub error: String,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    /// Instante (ms unix) da última leitura concluída — "Último Polling".
    pub last_poll_ms: Option<u64>,
    /// A última leitura terminou em 200 (chip "HTTP 200 OK" da 3.5).
    pub last_ok: bool,
    /// Aviso transitório de sucesso (exportação) — limpo no `refresh`.
    pub notice: String,
    /// Queda simulada pelo operador (botão do mockup): o erro rotulado
    /// aparece sem tocar a rede; `refresh` limpa e lê de verdade.
    pub simulate_outage: bool,
    receiver: Option<mpsc::Receiver<AgentsMsg>>,
}

impl AgentsState {
    /// Padrão do mockup 3.5: orquestrador do lab (editável).
    #[must_use]
    pub fn new() -> Self {
        Self {
            url: String::from("http://192.168.1.61:8080"),
            ..Self::default()
        }
    }

    /// Recarrega a lista em THREAD de trabalho (REQ/T-820-19; T-830-01);
    /// erro preserva a lista anterior e registra o motivo. Clique durante
    /// `busy` é ignorado. Limpa aviso/avaria simulada (pedido explícito).
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        self.notice.clear();
        self.simulate_outage = false;
        let url = self.url.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(list_agents(&url).map_err(|err| err.to_string()));
        });
        self.receiver = Some(rx);
        self.busy = true;
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(result) = rx.try_recv() {
                match result {
                    Ok(list) => {
                        self.list = list;
                        self.error.clear();
                        self.last_ok = true;
                    }
                    Err(detail) => {
                        self.error = detail;
                        self.last_ok = false;
                    }
                }
                self.last_poll_ms = Some(crate::machines::now_unix_ns() / 1_000_000);
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
    }

    /// Liga/desliga a queda simulada (sem rede; erro honesto rotulado).
    pub fn toggle_outage(&mut self) {
        self.simulate_outage = !self.simulate_outage;
        if self.simulate_outage {
            self.last_ok = false;
            self.error = String::from("queda simulada pelo operador (:8080 ignorado)");
        } else {
            self.error.clear();
        }
    }

    /// Zera a telemetria HTTP exibida (lista, erro e marcas).
    pub fn clear_stats(&mut self) {
        self.list.clear();
        self.error.clear();
        self.notice.clear();
        self.last_ok = false;
        self.last_poll_ms = None;
    }

    /// Snapshot JSON da lista atual em `dir`; retorna o caminho escrito.
    pub fn export_snapshot(&self, dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        let payload = serde_json::to_string_pretty(&self.list).map_err(std::io::Error::other)?;
        let name = format!(
            "studio_agents_{}.json",
            crate::machines::now_unix_ns() / 1_000_000_000
        );
        let path = dir.join(name);
        std::fs::write(&path, payload)?;
        Ok(path)
    }
}

/// Lista os agentes vivos (`GET /api/v1/agents`).
pub fn list_agents(base_url: &str) -> Result<Vec<AgentInfo>, AgentsError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|err| AgentsError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    #[derive(Deserialize)]
    struct List {
        agents: Vec<AgentInfo>,
    }
    client
        .get(format!("{url}/api/v1/agents"))
        .send()
        .map_err(|err| AgentsError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .error_for_status()
        .map_err(|err| AgentsError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .json::<List>()
        .map(|list| list.agents)
        .map_err(|err| AgentsError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_points_at_lab_orchestrator() {
        let state = AgentsState::new();
        assert_eq!(state.url, "http://192.168.1.61:8080");
        assert!(state.list.is_empty());
        assert!(!state.busy);
    }

    #[test]
    fn outage_toggle_and_clear_stats_are_honest() {
        let mut state = AgentsState::new();
        state.toggle_outage();
        assert!(state.simulate_outage);
        assert!(state.error.contains("simulada"));
        state.toggle_outage();
        assert!(!state.simulate_outage);
        assert!(state.error.is_empty());
        state.last_ok = true;
        state.clear_stats();
        assert!(!state.last_ok);
        assert!(state.last_poll_ms.is_none());
    }
}
