//! `studio-noded`: serve o protocolo administrativo em HTTP localhost.
//!
//! Porta via `STUDIO_NODE_PORT` (padrão 4317). Serviços próprios via
//! `STUDIO_NODE_SERVICES` (lista separada por vírgula; padrão `dds-agent`).
//! Persistência via `STUDIO_NODE_DB`; sem a variável, o padrão agora é
//! PERSISTENTE em `$HOME/.local/share/studio-node/operations.json`, criado
//! on demand (T-830-05). `HOME` ausente cai para memória volátil com aviso
//! explícito — nunca pânico.

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
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("studio-noded: porta {port} indisponivel em 127.0.0.1"))?;
    let services = owned_services();
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
