//! Parte de `overview` (extraída para manter os arquivos revisáveis).

use super::*;

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
    // 6º card do mockup 3.1: catálogo compartilhado (snapshot da autoridade).
    tiles.push(if !input.catalog_error.is_empty() {
        OverviewTile {
            title: String::from("Catálogo compartilhado"),
            summary: format!("não lido: {}", input.catalog_error),
            stale: true,
            health: TileHealth::Stale,
        }
    } else if input.catalog_loaded {
        OverviewTile {
            title: String::from("Catálogo compartilhado"),
            summary: format!(
                "{} registro(s) no snapshot · cursor {}",
                input.catalog_items, input.catalog_cursor
            ),
            stale: false,
            health: TileHealth::Ok,
        }
    } else {
        OverviewTile {
            title: String::from("Catálogo compartilhado"),
            summary: if input.discovery_target.is_some() {
                String::from("carregando snapshot do nó alvo…")
            } else {
                String::from("nunca lido — abra Catálogo e clique Ler snapshot")
            },
            stale: input.discovery_target.is_none(),
            health: if input.discovery_target.is_some() {
                TileHealth::Warn
            } else {
                TileHealth::Stale
            },
        }
    });
    tiles
}
