//! `studio-noded`: serve o protocolo administrativo em HTTP.
//!
//! Endereço via `STUDIO_NODE_BIND` (padrão `127.0.0.1`; use o IP da LAN
//! para administração remota por outra GUI — §34). Porta via
//! `STUDIO_NODE_PORT` (padrão 4317). Serviços próprios via
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
    let bind = bind_addr();
    let listener = tokio::net::TcpListener::bind((bind.as_str(), port))
        .await
        .with_context(|| format!("studio-noded: porta {port} indisponivel em {bind}"))?;
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

/// Endereço de escuta: `STUDIO_NODE_BIND`, padrão localhost (seguro por
/// padrão; administração remota exige valor explícito — §34).
fn bind_addr() -> String {
    let bind = std::env::var("STUDIO_NODE_BIND").unwrap_or_else(|_| String::from("127.0.0.1"));
    if bind.trim().is_empty() {
        String::from("127.0.0.1")
    } else {
        bind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bind_is_localhost() {
        let previous = std::env::var("STUDIO_NODE_BIND").ok();
        std::env::remove_var("STUDIO_NODE_BIND");
        let bind = bind_addr();
        restore_bind(previous);
        assert_eq!(bind, "127.0.0.1");
    }

    #[test]
    fn explicit_bind_is_honored_and_blank_falls_back() {
        let previous = std::env::var("STUDIO_NODE_BIND").ok();
        for (value, expected) in [
            ("192.168.1.62", "192.168.1.62"),
            ("0.0.0.0", "0.0.0.0"),
            ("   ", "127.0.0.1"),
        ] {
            std::env::set_var("STUDIO_NODE_BIND", value);
            assert_eq!(bind_addr(), expected);
        }
        restore_bind(previous);
    }

    fn restore_bind(previous: Option<String>) {
        match previous {
            Some(value) => std::env::set_var("STUDIO_NODE_BIND", value),
            None => std::env::remove_var("STUDIO_NODE_BIND"),
        }
    }
}
