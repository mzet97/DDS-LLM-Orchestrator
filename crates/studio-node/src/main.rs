//! `studio-noded`: serve o protocolo administrativo em HTTP localhost.
//!
//! Porta via `STUDIO_NODE_PORT` (padrão 4317). Serviços próprios via
//! `STUDIO_NODE_SERVICES` (lista separada por vírgula; padrão `dds-agent`).
//! Persistência opcional via `STUDIO_NODE_DB` (caminho do JSON do log).

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
    let state = match std::env::var("STUDIO_NODE_DB") {
        Ok(path) => NodeState::with_db(path.into(), services)
            .context("studio-noded: log persistido corrompido")?,
        Err(_) => NodeState::new(services),
    };
    axum::serve(listener, router(state))
        .await
        .context("servidor do no encerrou com erro")
}
