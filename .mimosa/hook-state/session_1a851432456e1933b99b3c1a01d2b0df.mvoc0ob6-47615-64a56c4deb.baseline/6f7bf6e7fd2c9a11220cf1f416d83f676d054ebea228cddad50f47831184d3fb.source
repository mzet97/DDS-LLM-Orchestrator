//! Descoberta automática de instalações (T-890-03, G-70/44): ao abrir o
//! Studio, um worker em background escuta o tópico `Studio.NodePresence`
//! (19º tópico) no domínio informado e mantém a lista de instalações vivas —
//! sem nenhum clique. Cada nó novo recebe probe automático de `/version`
//! com o token salvo em `~/.config/studio/hosts/<host>.token`, quando houver.
//!
//! Com a feature `dds`, [`DiscoveryState::start`] sobe o worker (thread +
//! runtime tokio próprio, padrão da GUI). Sem ela, o estado existe e a UI
//! explica como habilitar.

#[cfg(feature = "dds")]
use crate::machines::ProbeState;
use crate::machines::{now_unix_ns, ProbeStatus};
#[cfg(feature = "dds")]
use dds_contract::generated::dds_llm_orchestrator::AgentState;
#[cfg(feature = "dds")]
use dds_contract::generated::orchestrator::ServerStatus;

/// Uma instalação descoberta via `Studio.NodePresence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredNode {
    pub node_id: String,
    pub url: String,
    pub token_required: bool,
    /// Último heartbeat recebido (ns UNIX) — idade > lease (10 s) = morto.
    pub last_seen_unix_ns: u64,
    /// Probe automático de `/version` (preenchido quando o nó é novo).
    pub probe: Option<ProbeStatus>,
}

impl DiscoveredNode {
    /// Idade do último heartbeat em segundos (relógio local).
    #[must_use]
    pub fn age_secs(&self, now_unix_ns: u64) -> u64 {
        now_unix_ns.saturating_sub(self.last_seen_unix_ns) / 1_000_000_000
    }

    /// Vivo = heartbeat dentro do lease do tópico (10 s, dobrado por
    /// tolerância de relógio entre máquinas).
    #[must_use]
    pub fn is_alive(&self, now_unix_ns: u64) -> bool {
        self.age_secs(now_unix_ns) <= 20
    }
}

/// Agente vivo visto no `AgentRegistry` (linha da GUI — o tipo do fio fica
/// no worker; sem dependência da crate de contrato sem a feature `dds`).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRow {
    pub agent_id: String,
    pub model: String,
    pub slots_busy: u32,
    pub slots_total: u32,
    pub ema_latency_ms: f32,
    pub last_update_ns: u64,
}

/// Servidor de inferência vivo (`ServerStatus`) — llama-server/ponte no domínio.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerRow {
    pub server_id: String,
    pub model_loaded: String,
    pub slots_idle: i32,
    pub slots_processing: i32,
    pub ready: bool,
}

/// Converte o `AgentState` do fio para a linha da GUI (worker, feature dds).
#[cfg(feature = "dds")]
fn agent_row(state: &AgentState) -> AgentRow {
    AgentRow {
        agent_id: state.agent_id.clone(),
        model: state.model.clone(),
        slots_busy: state.slots_busy,
        slots_total: state.slots_total,
        ema_latency_ms: state.ema_latency_ms,
        last_update_ns: state.last_update_ns,
    }
}

/// Converte o `ServerStatus` do fio para a linha da GUI (worker, feature dds).
#[cfg(feature = "dds")]
fn server_row(status: &ServerStatus) -> ServerRow {
    ServerRow {
        server_id: status.server_id.clone(),
        model_loaded: status.model_loaded.clone(),
        slots_idle: status.slots_idle,
        slots_processing: status.slots_processing,
        ready: status.ready,
    }
}

/// Evento do worker para a UI: foto completa do sistema vivo no domínio.
#[derive(Debug, Clone, Default)]
pub struct DiscoverySnapshot {
    pub nodes: Vec<DiscoveredNode>,
    /// Agentes vivos no `AgentRegistry` (heartbeats DDS).
    pub agents: Vec<AgentRow>,
    /// Servidores de inferência vivos (`ServerStatus` — llama-server/ponte).
    pub servers: Vec<ServerRow>,
}

/// Estado do painel de descoberta (thread-safe via mpsc — padrão da GUI).
#[derive(Default)]
pub struct DiscoveryState {
    pub domain: u32,
    pub nodes: Vec<DiscoveredNode>,
    /// Agentes vivos no domínio (alimentado pelo worker).
    pub agents: Vec<AgentRow>,
    /// Servidores de inferência vivos no domínio (ServerStatus).
    pub servers: Vec<ServerRow>,
    /// `true` até o primeiro snapshot chegar (worker ainda escaneando).
    pub scanning: bool,
    pub error: String,
    /// Índice do nó selecionado como ALVO ÚNICO da GUI (T-890-03): todos os
    /// painéis com URL de nó apontam para ele; trocar aqui troca em todo lugar.
    pub selected: Option<usize>,
    #[cfg(feature = "dds")]
    receiver: Option<std::sync::mpsc::Receiver<DiscoverySnapshot>>,
}

impl DiscoveryState {
    /// Estado sem worker (feature `dds` desligada — a UI explica).
    #[must_use]
    pub fn new_disabled(domain: u32) -> Self {
        Self {
            domain,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.scanning
    }

    /// Drena o snapshot mais recente do worker (chamar por frame).
    pub fn poll(&mut self) {
        // Drena primeiro para um vetor próprio: `observe_system` toma
        // `&mut self` e não pode rodar sob o empréstimo do `receiver`.
        #[cfg(feature = "dds")]
        {
            let mut snapshots = Vec::new();
            if let Some(rx) = &self.receiver {
                snapshots.extend(rx.try_iter());
            }
            for snapshot in snapshots {
                self.observe_system(snapshot.nodes, snapshot.agents, snapshot.servers);
                self.error.clear();
            }
        }
    }

    /// Observa um snapshot direto (mesma transição do `poll`, testável).
    pub fn observe(&mut self, nodes: Vec<DiscoveredNode>) {
        // Remapeia a seleção por IDENTIDADE (P1): a lista chega ordenada
        // por `node_id` do worker — um nó novo antes do selecionado
        // deslocaria o índice e `selected_url()` apontaria para outro nó
        // sem nenhum clique. Nó sumido → seleção limpa (a auto-seleção do
        // `main` escolhe outro online na sequência).
        let selected_id = self
            .selected
            .and_then(|index| self.nodes.get(index))
            .map(|node| node.node_id.clone());
        self.nodes = nodes;
        self.selected = selected_id.and_then(|id| self.nodes.iter().position(|n| n.node_id == id));
        self.scanning = false;
    }

    /// Observa o inventário completo (testável sem worker).
    pub fn observe_system(
        &mut self,
        nodes: Vec<DiscoveredNode>,
        agents: Vec<AgentRow>,
        servers: Vec<ServerRow>,
    ) {
        self.observe(nodes);
        self.agents = agents;
        self.servers = servers;
    }

    /// URL do nó selecionado (`None` sem seleção ou seleção fora da lista).
    #[must_use]
    pub fn selected_url(&self) -> Option<String> {
        let index = self.selected?;
        self.nodes
            .get(index)
            .map(|node| node.url.clone())
            .filter(|url| !url.trim().is_empty())
    }

    /// Seleciona pelo índice (clamp implícito em `selected_url`).
    pub fn select(&mut self, index: usize) {
        self.selected = Some(index);
    }

    /// Auto-seleção do primeiro nó ONLINE (chamado quando a lista muda e
    /// ainda não há seleção): abre o Studio, o primeiro nó vivo vira o
    /// alvo — os painéis conectam sem nenhum clique.
    pub fn autoselect_first_online(&mut self) -> bool {
        if self.selected.is_some() {
            return false;
        }
        let now = now_unix_ns();
        let index = self.nodes.iter().position(|node| {
            node.is_alive(now)
                && matches!(
                    node.probe.as_ref().map(|p| p.state),
                    Some(crate::machines::ProbeState::Online)
                )
        });
        if let Some(index) = index {
            self.selected = Some(index);
            true
        } else {
            false
        }
    }
}

mod discovery_merge;
#[cfg(feature = "dds")]
mod discovery_worker;

#[cfg(any(test, feature = "dds"))]
pub use discovery_merge::merge_presence;
pub use discovery_merge::token_for_url;
#[cfg(any(test, feature = "dds"))]
use discovery_merge::{retain_fresh, HEARTBEAT_LEASE_NS};
#[cfg(feature = "dds")]
use discovery_worker::run_worker;

#[cfg(test)]
mod tests;
