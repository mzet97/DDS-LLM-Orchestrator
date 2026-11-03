//! Visão geral do Studio: agregado somente leitura (§9.2, tela obrigatória).
//!
//! Compõe estados que já existem (nó, serviços, agentes, modelos,
//! inferência) em cartões honestos: cada cartão diz a fonte e marca
//! `stale` quando a fonte nunca foi lida ou falhou. Nada é inventado.

use studio_node::server::ServiceStatus;

use crate::agents::AgentInfo;
use crate::origin::NodeSummary;

/// Estado visual do cartão (define a cor da borda/ícone na GUI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TileHealth {
    /// Fonte viva com dados.
    Ok,
    /// Aviso (parcial: ex. auto-carga em andamento).
    Warn,
    /// Fonte apagada/falha (honesto: nunca dado inventado).
    #[default]
    Stale,
}

/// Um cartão da visão geral: título, resumo e se a fonte está desatualizada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewTile {
    pub title: String,
    pub summary: String,
    pub stale: bool,
    /// Estado semântico derivado (Default = Stale, compat com construtores antigos).
    pub health: TileHealth,
}

/// Entradas observáveis para o agregado (emprestadas dos estados reais).
pub struct OverviewInput<'a> {
    pub node: Option<&'a NodeSummary>,
    pub node_error: &'a str,
    pub services: &'a [ServiceStatus],
    pub services_error: &'a str,
    pub agents: &'a [AgentInfo],
    pub agents_error: &'a str,
    pub models_total: usize,
    pub models_hashed: usize,
    pub inference_proof: &'a str,
    /// T-890-03: descoberta automática (nós/agentes/inferência do domínio).
    pub discovery_nodes: usize,
    pub discovery_agents: usize,
    pub discovery_servers: usize,
    pub discovery_target: Option<&'a str>,
    /// Catálogo compartilhado (6º card do mockup 3.1): snapshot da autoridade.
    pub catalog_items: usize,
    pub catalog_cursor: u64,
    pub catalog_loaded: bool,
    pub catalog_error: &'a str,
}

/// Resume os estados em cartões; vazio/falha vira cartão `stale`, nunca dado.
mod overview_summarize;

pub use overview_summarize::summarize;

#[cfg(test)]
mod tests;
