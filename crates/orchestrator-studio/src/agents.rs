//! Agentes no Studio: leitura viva do orquestrador (`GET /api/v1/agents`).
//!
//! Números crus do backend, sem semáforo inventado: saúde, slots e
//! contadores aparecem como o orquestrador anuncia.

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

/// Estado do painel de agentes: URL, última lista e último erro.
#[derive(Debug, Clone)]
pub struct AgentsState {
    pub url: String,
    pub list: Vec<AgentInfo>,
    pub error: String,
}

impl AgentsState {
    /// Padrão honesto: orquestrador local.
    #[must_use]
    pub fn new() -> Self {
        Self {
            url: String::from("http://127.0.0.1:8085"),
            list: Vec::new(),
            error: String::new(),
        }
    }

    /// Recarrega a lista; erro preserva a lista anterior e registra o motivo.
    pub fn refresh(&mut self) {
        match list_agents(&self.url.clone()) {
            Ok(list) => {
                self.list = list;
                self.error.clear();
            }
            Err(err) => {
                self.error = err.to_string();
            }
        }
    }
}

impl Default for AgentsState {
    fn default() -> Self {
        Self::new()
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
