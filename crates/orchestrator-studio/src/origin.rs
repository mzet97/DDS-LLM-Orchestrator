//! Origem remota do Studio: leitura viva do `studio-node` via HTTP.
//!
//! O GUI não inventa linhas — [`NodeSummary`] vem de `GET /version` +
//! `GET /operations` do nó, com checagem de compatibilidade de protocolo
//! (§31: par incompatível bloqueia, nunca aplica).
//!
//! Cliente bloqueante de propósito: a thread de UI do eframe não tem runtime
//! async; nunca chamar dentro de runtime tokio sem `spawn_blocking`.

use studio_node::operations::OpRecord;
use studio_node::protocol::{ProtocolVersion, NODE_PROTOCOL_VERSION};
use thiserror::Error;

/// Resumo observável do nó: versão anunciada e operações registradas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSummary {
    pub version: ProtocolVersion,
    pub operations: Vec<OpRecord>,
}

/// Erros da origem remota (fronteira GUI ↔ nó).
#[derive(Debug, Error)]
pub enum OriginError {
    /// Nó inalcançável ou resposta inválida.
    #[error("falha ao ler o no em {url}: {detail}")]
    Unreachable { url: String, detail: String },
    /// Protocolo do nó incompatível com o Studio: bloqueia a leitura.
    #[error("no com protocolo incompativel: {peer:?}")]
    Incompatible { peer: ProtocolVersion },
}

/// Lê o resumo vivo do nó em `base_url` (ex. `http://127.0.0.1:4317`).
pub fn fetch_node_summary(base_url: &str) -> Result<NodeSummary, OriginError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|err| OriginError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let get = |path: &str| {
        client
            .get(format!("{url}{path}"))
            .send()
            .map_err(|err| OriginError::Unreachable {
                url: String::from(url),
                detail: err.to_string(),
            })?
            .error_for_status()
            .map_err(|err| OriginError::Unreachable {
                url: String::from(url),
                detail: err.to_string(),
            })
    };
    let version: ProtocolVersion =
        get("/version")?
            .json()
            .map_err(|err| OriginError::Unreachable {
                url: String::from(url),
                detail: err.to_string(),
            })?;
    if !NODE_PROTOCOL_VERSION.accepts(version) {
        return Err(OriginError::Incompatible { peer: version });
    }
    let operations: Vec<OpRecord> =
        get("/operations")?
            .json()
            .map_err(|err| OriginError::Unreachable {
                url: String::from(url),
                detail: err.to_string(),
            })?;
    Ok(NodeSummary {
        version,
        operations,
    })
}
