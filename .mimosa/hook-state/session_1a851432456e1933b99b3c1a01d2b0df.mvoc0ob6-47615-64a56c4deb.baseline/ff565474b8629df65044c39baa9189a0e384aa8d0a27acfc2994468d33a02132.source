//! Origem remota do Studio: leitura viva do `studio-node` via HTTP.
//!
//! O GUI não inventa linhas — [`NodeSummary`] vem de `GET /version` +
//! `GET /operations` do nó, com checagem de compatibilidade de protocolo
//! (§31: par incompatível bloqueia, nunca aplica).
//!
//! Token de acesso (T-840-03a, nó conforme T-840-01): com `STUDIO_NODE_TOKEN`
//! no nó, todas as rotas — exceto `GET /version` — exigem
//! `Authorization: Bearer <token>`. O token é opcional aqui (`Option<&str>`)
//! e vive SÓ na memória da GUI: nunca é serializado, persistido nem impresso
//! (segredos por referência — RNF-04, SDD §28). 401 é erro tipado distinto
//! ([`OriginError::Unauthorized`]) para separar "fora do ar" de
//! "credencial recusada".
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
    /// Nó respondeu 401: token ausente ou inválido (T-840-01; T-840-03a).
    #[error("token recusado (401)")]
    Unauthorized,
    /// Protocolo do nó incompatível com o Studio: bloqueia a leitura.
    #[error("no com protocolo incompativel: {peer:?}")]
    Incompatible { peer: ProtocolVersion },
}

/// Converte o texto de um campo de UI em token opcional: vazio/branco = sem
/// token (comportamento padrão pré-T-840-03a, testes existentes preservados).
#[must_use]
pub fn optional_token(token: &str) -> Option<&str> {
    let trimmed = token.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Aplica `Authorization: Bearer <token>` quando há token (T-840-03a).
pub(crate) fn authorized(
    request: reqwest::blocking::RequestBuilder,
    token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    match token {
        Some(token) => request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}")),
        None => request,
    }
}

/// Decodifica a resposta como JSON; 401 é erro tipado distinto (T-840-03a),
/// demais status de erro viram [`OriginError::Unreachable`].
fn decode<T: serde::de::DeserializeOwned>(
    url: &str,
    response: reqwest::blocking::Response,
) -> Result<T, OriginError> {
    if response.status().as_u16() == 401 {
        return Err(OriginError::Unauthorized);
    }
    response
        .error_for_status()
        .map_err(|err| OriginError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .json()
        .map_err(|err| OriginError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

/// Lê o resumo vivo do nó em `base_url` (ex. `http://127.0.0.1:4317`), sem
/// token — mantida para o modo localhost (T-840-03a: wrapper de
/// [`fetch_node_summary_with_token`] com `token = None`).
pub fn fetch_node_summary(base_url: &str) -> Result<NodeSummary, OriginError> {
    fetch_node_summary_with_token(base_url, None)
}

/// Como [`fetch_node_summary`], com token opcional (T-840-03a).
pub fn fetch_node_summary_with_token(
    base_url: &str,
    token: Option<&str>,
) -> Result<NodeSummary, OriginError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        // Keepalive TCP explícito (a 3.2 anuncia `TCP_KEEPALIVE: ATIVO`).
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| OriginError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let get = |path: &str| {
        authorized(client.get(format!("{url}{path}")), token)
            .send()
            .map_err(|err| OriginError::Unreachable {
                url: String::from(url),
                detail: err.to_string(),
            })
    };
    let version: ProtocolVersion = decode(url, get("/version")?)?;
    if !NODE_PROTOCOL_VERSION.accepts(version) {
        return Err(OriginError::Incompatible { peer: version });
    }
    let operations: Vec<OpRecord> = decode(url, get("/operations")?)?;
    Ok(NodeSummary {
        version,
        operations,
    })
}

/// Sonda de Máquinas/descoberta (T-840-03c): `GET /version` + validação do
/// token em `GET /operations` (protegido), com token opcional (T-840-03a).
/// `/version` é aberto (T-840-01) — sem a segunda chamada, token errado
/// ainda retornaria `Ok` e a GUI marcaria Online, com `AuthPending`
/// inalcançável (P2). Só o STATUS de `/operations` importa: 401 vira
/// [`OriginError::Unauthorized`]; o corpo nem é lido. Sem checagem de
/// compatibilidade aqui — a versão anunciada é exibida como veio; quem
/// decide é o painel.
pub fn fetch_node_version_with_token(
    base_url: &str,
    token: Option<&str>,
) -> Result<ProtocolVersion, OriginError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        // Keepalive TCP explícito (a 3.2 anuncia `TCP_KEEPALIVE: ATIVO`).
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| OriginError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let get = |path: &str| {
        authorized(client.get(format!("{url}{path}")), token)
            .send()
            .map_err(|err| OriginError::Unreachable {
                url: String::from(url),
                detail: err.to_string(),
            })
    };
    let version: ProtocolVersion = decode(url, get("/version")?)?;
    if get("/operations")?.status().as_u16() == 401 {
        return Err(OriginError::Unauthorized);
    }
    Ok(version)
}
