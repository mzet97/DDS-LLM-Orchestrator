//! `studio-noded`: serve o protocolo administrativo em HTTP.
//!
//! Porta via `STUDIO_NODE_PORT` (padrão 4317) e bind via `STUDIO_NODE_BIND`
//! (padrão `127.0.0.1`; em LAN é **obrigatório** `STUDIO_NODE_TOKEN`, que
//! protege todas as rotas exceto `GET /version` — T-840-01). Serviços
//! próprios via `STUDIO_NODE_SERVICES` (lista separada por vírgula; padrão
//! `dds-agent`). Persistência via `STUDIO_NODE_DB`; sem a variável, o padrão
//! agora é PERSISTENTE em `$HOME/.local/share/studio-node/operations.json`,
//! criado on demand (T-830-05). `HOME` ausente cai para memória volátil com
//! aviso explícito — nunca pânico.
//!
//! Presença DDS (T-890): com a feature `dds` e `STUDIO_NODE_DDS_DOMAIN`
//! definida, o nó publica heartbeat `Studio.NodePresence` a cada 5 s
//! (descoberta DDS-nativa das instalações do Studio; mDNS eliminado). Sem a
//! env/feature, HTTP-only.

use std::path::PathBuf;

use anyhow::{Context, Result};
use studio_node::server::{router, NodeState};

fn owned_services() -> Vec<String> {
    std::env::var("STUDIO_NODE_SERVICES")
        .unwrap_or_else(|_| String::from("dds-agent"))
        .split(',')
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Resolução do caminho do log de operações (T-830-05).
#[derive(Debug, PartialEq, Eq)]
enum ResolvedDb {
    /// Persiste no caminho dado (env explícito ou padrão derivado de HOME).
    Persistent(PathBuf),
    /// Volátil em memória, com o motivo para o aviso de boot.
    Volatile { reason: String },
}

/// Ordem de resolução: `STUDIO_NODE_DB` vence; sem env,
/// `$HOME/.local/share/studio-node/operations.json`; sem `HOME`, volátil
/// (função pura para teste direto — T-830-05).
fn resolve_db(explicit: Option<String>, home: Option<String>) -> ResolvedDb {
    match explicit {
        Some(path) => ResolvedDb::Persistent(PathBuf::from(path)),
        None => match home {
            Some(home) => ResolvedDb::Persistent(
                PathBuf::from(home).join(".local/share/studio-node/operations.json"),
            ),
            None => ResolvedDb::Volatile {
                reason: String::from("HOME ausente no ambiente"),
            },
        },
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let port: u16 = std::env::var("STUDIO_NODE_PORT")
        .unwrap_or_else(|_| String::from("4317"))
        .parse()
        .context("STUDIO_NODE_PORT deve ser um numero de porta")?;
    // T-840-01: bind configurável (LAN exige STUDIO_NODE_TOKEN; localhost é o
    // padrão histórico e segue seguro sem token).
    let bind: std::net::IpAddr = std::env::var("STUDIO_NODE_BIND")
        .unwrap_or_else(|_| String::from("127.0.0.1"))
        .parse()
        .context("STUDIO_NODE_BIND deve ser um endereco IP")?;
    let listener = tokio::net::TcpListener::bind((bind, port))
        .await
        .with_context(|| format!("studio-noded: porta {port} indisponivel em {bind}"))?;
    let services = owned_services();
    // T-890: token lido antes da construção do estado — a presença DDS
    // anuncia se a instalação exige token (o mesmo valor valida as rotas
    // abaixo).
    let token = std::env::var("STUDIO_NODE_TOKEN")
        .ok()
        .filter(|token| !token.is_empty());
    // T-890: presença DDS opcional (19º tópico `Studio.NodePresence`) —
    // `STUDIO_NODE_DDS_DOMAIN` ativa; sem a env o nó segue HTTP-only. Falha
    // de DDS NUNCA derruba o nó administrativo (spawn best-effort).
    #[cfg(feature = "dds")]
    studio_node::presence::spawn_from_env(bind, port, token.is_some(), &services);
    #[cfg(not(feature = "dds"))]
    if studio_node::presence::dds_domain_from_env(
        std::env::var(studio_node::presence::ENV_DDS_DOMAIN).ok(),
    )
    .is_some()
    {
        eprintln!(
            "studio-noded: aviso: {} definida, mas o binario foi compilado sem a \
             feature `dds` — presenca DDS desativada",
            studio_node::presence::ENV_DDS_DOMAIN
        );
    }
    let resolved = resolve_db(
        std::env::var("STUDIO_NODE_DB").ok(),
        std::env::var("HOME").ok(),
    );
    let state = match resolved {
        ResolvedDb::Persistent(path) => {
            // Diretório criado on demand; falha de disco degrada para
            // volátil com aviso (nó continua útil, nunca pânico).
            let persisted = match path.parent() {
                Some(parent) => std::fs::create_dir_all(parent)
                    .map_err(|err| format!("{}: {err}", parent.display())),
                None => Err(String::from("caminho do log sem diretório pai")),
            };
            match persisted {
                Ok(()) => NodeState::with_db(path.clone(), services).with_context(|| {
                    format!(
                        "studio-noded: log persistido corrompido em {}",
                        path.display()
                    )
                })?,
                Err(detail) => {
                    eprintln!(
                        "studio-noded: aviso: sem persistencia ({detail}); log volatil em memoria"
                    );
                    NodeState::new(services)
                }
            }
        }
        ResolvedDb::Volatile { reason } => {
            eprintln!(
                "studio-noded: aviso: log volatil em memoria ({reason}); \
                 defina HOME ou STUDIO_NODE_DB para persistir"
            );
            NodeState::new(services)
        }
    };
    // T-840-01: token opcional; bind fora de 127.0.0.1 sem token é recusado
    // (expor operações administrativas na LAN sem auth nunca é aceitável).
    // (Leitura da env hoisted acima — a presença DDS anuncia token_required.)
    let state = match (&bind, token.as_deref()) {
        (std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), _)
        | (std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), _) => match token {
            Some(token) => state.with_token(&token),
            None => state,
        },
        (_, Some(token)) => {
            if token.len() < 16 {
                anyhow::bail!(
                    "studio-noded: STUDIO_NODE_TOKEN muito curto (>=16 caracteres) para bind em {bind}"
                );
            }
            state.with_token(token)
        }
        (_, None) => {
            anyhow::bail!(
                "studio-noded: bind em {bind} exige STUDIO_NODE_TOKEN (>=16 caracteres); \
                 use STUDIO_NODE_BIND=127.0.0.1 para modo localhost sem token"
            );
        }
    };
    axum::serve(listener, router(state))
        .await
        .context("servidor do no encerrou com erro")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins_over_home_default() {
        let resolved = resolve_db(
            Some(String::from("/var/lib/studio/log.json")),
            Some(String::from("/home/alguem")),
        );
        assert_eq!(
            resolved,
            ResolvedDb::Persistent(PathBuf::from("/var/lib/studio/log.json"))
        );
    }

    #[test]
    fn home_derives_xdg_default_path() {
        // HOME presente e sem env: $HOME/.local/share/studio-node/operations.json.
        let resolved = resolve_db(None, Some(String::from("/home/alguem")));
        assert_eq!(
            resolved,
            ResolvedDb::Persistent(PathBuf::from(
                "/home/alguem/.local/share/studio-node/operations.json"
            ))
        );
        let ResolvedDb::Persistent(path) = resolved else {
            panic!("HOME presente deve resolver caminho persistente");
        };
        assert!(path.ends_with(".local/share/studio-node/operations.json"));
    }

    #[test]
    fn missing_home_falls_back_to_volatile_with_reason() {
        let resolved = resolve_db(None, None);
        let ResolvedDb::Volatile { reason } = resolved else {
            panic!("sem HOME e sem env deve ser volátil");
        };
        assert!(reason.contains("HOME"));
    }
}
