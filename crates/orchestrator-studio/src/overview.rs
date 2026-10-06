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
}

/// Resume os estados em cartões; vazio/falha vira cartão `stale`, nunca dado.
#[must_use]
pub fn summarize(input: &OverviewInput<'_>) -> Vec<OverviewTile> {
    let mut tiles = Vec::with_capacity(5);
    tiles.push(match input.node {
        Some(summary) => OverviewTile {
            title: String::from("Nó"),
            summary: format!(
                "conectado · protocolo {}.{} · {} operação(ões)",
                summary.version.major,
                summary.version.minor,
                summary.operations.len()
            ),
            stale: false,
            health: TileHealth::Ok,
        },
        None => OverviewTile {
            title: String::from("Nó"),
            summary: if input.node_error.is_empty() {
                match input.discovery_target {
                    // Auto-carga em andamento: alvo definido pela descoberta.
                    Some(target) => format!("conectando ao alvo {target}…"),
                    None => String::from("não conectado — abra Nó studio-node e Conecte"),
                }
            } else {
                format!("não conectado: {}", input.node_error)
            },
            stale: input.discovery_target.is_none(),
            health: if input.discovery_target.is_none() {
                TileHealth::Stale
            } else {
                TileHealth::Warn
            },
        },
    });
    tiles.push(
        if input.services_error.is_empty() && !input.services.is_empty() {
            let ready = input.services.iter().filter(|item| item.active).count();
            let divergent = input
                .services
                .iter()
                .filter(|item| item.wanted.is_some_and(|wanted| wanted != item.active))
                .count();
            OverviewTile {
                title: String::from("Serviços"),
                summary: format!(
                    "{} pronto(s) de {} · {} divergente(s) pretendido×efetivo",
                    ready,
                    input.services.len(),
                    divergent
                ),
                stale: false,
                health: TileHealth::Ok,
            }
        } else if !input.services_error.is_empty() {
            OverviewTile {
                title: String::from("Serviços"),
                summary: format!("não lidos: {}", input.services_error),
                stale: true,
                health: TileHealth::Stale,
            }
        } else {
            OverviewTile {
                title: String::from("Serviços"),
                summary: if input.discovery_target.is_some() {
                    String::from("carregando plano do nó alvo…")
                } else {
                    String::from("nunca lidos — abra Serviços e clique Ler plano")
                },
                stale: true,
                health: if input.discovery_target.is_some() {
                    TileHealth::Warn
                } else {
                    TileHealth::Stale
                },
            }
        },
    );
    tiles.push(
        if input.agents_error.is_empty() && !input.agents.is_empty() {
            let slots_busy: u32 = input.agents.iter().map(|item| item.slots_busy).sum();
            let slots_total: u32 = input.agents.iter().map(|item| item.slots_total).sum();
            OverviewTile {
                title: String::from("Agentes"),
                summary: format!(
                    "{} agente(s) · slots {}/{} ocupados",
                    input.agents.len(),
                    slots_busy,
                    slots_total
                ),
                stale: false,
                health: TileHealth::Ok,
            }
        } else if !input.agents_error.is_empty() {
            OverviewTile {
                title: String::from("Agentes"),
                summary: if input.discovery_agents > 0 {
                    format!(
                        "{} agente(s) no domínio via descoberta DDS",
                        input.discovery_agents
                    )
                } else {
                    format!("não lidos: {}", input.agents_error)
                },
                stale: input.discovery_agents == 0,
                health: if input.discovery_agents > 0 {
                    TileHealth::Ok
                } else {
                    TileHealth::Stale
                },
            }
        } else {
            OverviewTile {
                title: String::from("Agentes"),
                summary: if input.discovery_agents > 0 {
                    format!(
                        "{} agente(s) no domínio via descoberta DDS",
                        input.discovery_agents
                    )
                } else {
                    String::from("nenhum agente no domínio nem no orquestrador")
                },
                stale: input.discovery_agents == 0,
                health: if input.discovery_agents > 0 {
                    TileHealth::Ok
                } else {
                    TileHealth::Stale
                },
            }
        },
    );
    tiles.push(OverviewTile {
        title: String::from("Modelos"),
        summary: if input.models_total == 0 {
            String::from("nenhum .gguf listado — abra Modelos GGUF e Inventarie")
        } else {
            format!(
                "{} arquivo(s) · {} com SHA-256",
                input.models_total, input.models_hashed
            )
        },
        stale: input.models_total == 0,
        health: if input.models_total == 0 {
            TileHealth::Stale
        } else {
            TileHealth::Ok
        },
    });
    tiles.push(OverviewTile {
        title: String::from("Inferência"),
        summary: if input.inference_proof.is_empty() {
            String::from("sem prova de geração ainda — abra Subir inferência")
        } else {
            format!("última prova: {}", input.inference_proof)
        },
        stale: input.inference_proof.is_empty() && input.discovery_servers == 0,
        health: if !input.inference_proof.is_empty() {
            TileHealth::Ok
        } else if input.discovery_servers > 0 {
            TileHealth::Warn
        } else {
            TileHealth::Stale
        },
    });
    tiles
}

#[cfg(test)]
mod tests {
    use studio_node::protocol::{ProtocolVersion, NODE_PROTOCOL_VERSION};

    use super::*;

    fn input<'a>(
        node: Option<&'a NodeSummary>,
        services: &'a [ServiceStatus],
        agents: &'a [AgentInfo],
    ) -> OverviewInput<'a> {
        OverviewInput {
            node,
            node_error: "",
            services,
            services_error: "",
            agents,
            agents_error: "",
            models_total: 0,
            models_hashed: 0,
            inference_proof: "",
            discovery_nodes: 0,
            discovery_agents: 0,
            discovery_servers: 0,
            discovery_target: None,
        }
    }

    #[test]
    fn empty_everything_is_stale_never_fake() {
        let data = input(None, &[], &[]);

        let tiles = summarize(&data);

        assert_eq!(tiles.len(), 5);
        assert!(tiles.iter().all(|tile| tile.stale));
    }

    #[test]
    fn counts_come_from_real_slices() {
        let node = NodeSummary {
            version: ProtocolVersion {
                major: NODE_PROTOCOL_VERSION.major,
                minor: NODE_PROTOCOL_VERSION.minor,
            },
            operations: Vec::new(),
        };
        let services = vec![
            ServiceStatus {
                service: String::from("a"),
                wanted: Some(true),
                active: true,
            },
            ServiceStatus {
                service: String::from("b"),
                wanted: Some(true),
                active: false,
            },
        ];
        let agents = vec![AgentInfo {
            agent_id: String::from("ag-1"),
            model: String::new(),
            specialization: String::new(),
            hostname: String::new(),
            health: 100,
            slots_busy: 1,
            slots_total: 4,
            completed_total: 0,
            failed_total: 0,
            ema_latency_ms: 0.0,
        }];
        let mut data = input(Some(&node), &services, &agents);
        data.models_total = 15;
        data.models_hashed = 4;
        data.inference_proof = "OK";

        let tiles = summarize(&data);

        assert!(tiles.iter().all(|tile| !tile.stale));
        let services_tile = tiles
            .iter()
            .find(|tile| tile.title == "Serviços")
            .expect("servicos");
        assert!(services_tile.summary.contains("1 pronto(s) de 2"));
        assert!(services_tile.summary.contains("1 divergente(s)"));
        let agents_tile = tiles
            .iter()
            .find(|tile| tile.title == "Agentes")
            .expect("agentes");
        assert!(agents_tile.summary.contains("slots 1/4"));
    }

    #[test]
    fn source_errors_stay_visible() {
        let mut data = input(None, &[], &[]);
        data.node_error = "conexão recusada";
        data.services_error = "timeout";

        let tiles = summarize(&data);

        let node = tiles.iter().find(|tile| tile.title == "Nó").expect("no");
        assert!(node.summary.contains("conexão recusada"));
        let services = tiles
            .iter()
            .find(|tile| tile.title == "Serviços")
            .expect("servicos");
        assert!(services.summary.contains("timeout"));
    }
}
