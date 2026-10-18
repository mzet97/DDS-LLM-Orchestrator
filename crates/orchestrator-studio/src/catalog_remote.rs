//! Catálogo compartilhado no Studio: cliente da autoridade do nó.
//!
//! Publicação/exclusão condicionais com base explícita; snapshot e eventos
//! para acompanhamento. Conflito nunca é silencioso: vira texto com a
//! revisão vigente e dispara releitura do snapshot.
//!
//! Token de acesso (T-840-03a, nó conforme T-840-01): autoridades em bind
//! não-localhost exigem `Authorization: Bearer <token>` em TODAS as rotas do
//! catálogo. O token é opcional (`*_with_token`; wrappers sem token mantêm o
//! comportamento pré-T-840-03a) e vive SÓ na memória da GUI — nunca é
//! serializado no valor do catálogo nem impresso em `Debug` (segredos por
//! referência — RNF-04, SDD §28).
//!
//! Chamadas HTTP bloqueantes rodam em THREAD de trabalho (padrão
//! `models.rs`: thread + mpsc + `poll` por frame) — a thread de UI nunca
//! bloqueia (REQ/T-820-19).

use std::sync::mpsc;
use studio_core::catalog::{DefinitionId, Event, EventKind, Snapshot};
use thiserror::Error;

use crate::origin::{authorized, optional_token};

/// Erros do catálogo compartilhado (fronteira GUI ↔ autoridade).
#[derive(Debug, Error)]
pub enum SharedCatalogError {
    /// Base obsoleta: nada aplicado; `current` é a vigente.
    #[error("base obsoleta; vigente: {current:?}")]
    Conflict { current: Option<u64> },
    /// Id com tombstone: recriar exige identidade nova.
    #[error("id removido; recriar exige identidade nova")]
    Tombstoned,
    /// Cursor fora da retenção: ressincronizar pelo snapshot.
    #[error("cursor expirado; refazer snapshot")]
    CursorExpired,
    /// Autoridade respondeu 401: token ausente ou inválido
    /// (T-840-01; T-840-03a).
    #[error("token recusado (401)")]
    Unauthorized,
    /// Autoridade inalcançável ou fora do contrato.
    #[error("falha no catalogo em {url}: {detail}")]
    Unreachable { url: String, detail: String },
}

fn client(url: &str) -> Result<reqwest::blocking::Client, SharedCatalogError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|err| SharedCatalogError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

/// Snapshot consistente da autoridade (sem token; modo localhost).
pub fn fetch_snapshot(base_url: &str) -> Result<Snapshot, SharedCatalogError> {
    fetch_snapshot_with_token(base_url, None)
}

/// Como [`fetch_snapshot`], com token opcional (T-840-03a).
pub fn fetch_snapshot_with_token(
    base_url: &str,
    token: Option<&str>,
) -> Result<Snapshot, SharedCatalogError> {
    let url = base_url.trim_end_matches('/');
    let fail = |detail: String| SharedCatalogError::Unreachable {
        url: String::from(url),
        detail,
    };
    let response = authorized(
        client(url)
            .map_err(|_| fail(String::from("cliente http")))?
            .get(format!("{url}/catalog/snapshot")),
        token,
    )
    .send()
    .map_err(|err| fail(err.to_string()))?;
    match response.status().as_u16() {
        200 => response
            .json::<Snapshot>()
            .map_err(|err| fail(err.to_string())),
        401 => Err(SharedCatalogError::Unauthorized),
        status => Err(fail(format!("status inesperado: {status}"))),
    }
}

/// Publica condicionalmente; retorna a revisão resultante (sem token).
pub fn publish(
    base_url: &str,
    id: &str,
    base: Option<u64>,
    value: &str,
) -> Result<u64, SharedCatalogError> {
    publish_with_token(base_url, id, base, value, None)
}

/// Como [`publish`], com token opcional (T-840-03a).
pub fn publish_with_token(
    base_url: &str,
    id: &str,
    base: Option<u64>,
    value: &str,
    token: Option<&str>,
) -> Result<u64, SharedCatalogError> {
    mutate(base_url, "publish", id, base, Some(value), token)
}

/// Exclui com tombstone a partir da base (sem token).
pub fn delete(base_url: &str, id: &str, base: u64) -> Result<u64, SharedCatalogError> {
    delete_with_token(base_url, id, base, None)
}

/// Como [`delete`], com token opcional (T-840-03a).
pub fn delete_with_token(
    base_url: &str,
    id: &str,
    base: u64,
    token: Option<&str>,
) -> Result<u64, SharedCatalogError> {
    mutate(base_url, "delete", id, Some(base), None, token)
}

fn mutate(
    base_url: &str,
    action: &str,
    id: &str,
    base: Option<u64>,
    value: Option<&str>,
    token: Option<&str>,
) -> Result<u64, SharedCatalogError> {
    let url = base_url.trim_end_matches('/');
    let fail = |detail: String| SharedCatalogError::Unreachable {
        url: String::from(url),
        detail,
    };
    let mut body = serde_json::json!({"id": id, "base": base});
    if action == "publish" {
        body["value"] = serde_json::json!(value.unwrap_or_default());
        // REQ-801 (T-830-03): o nó agora recusa geração regressiva/repetida.
        // A GUI não conhece a geração vigente (o snapshot não a carrega), mas
        // derivá-la da base mantém a monotonia: criação semeia 0; atualizar
        // a partir de r declara r+1 — estritamente maior que a geração da
        // atualização anterior (r-1 → r). Conflito de geração real (outro
        // editor semeou geração maior) vira erro honesto na UI.
        body["generation"] = serde_json::json!(base.map_or(0, |base| base + 1));
    }
    let response = authorized(
        client(url)
            .map_err(|_| fail(String::from("cliente http")))?
            .post(format!("{url}/catalog/{action}")),
        token,
    )
    .json(&body)
    .send()
    .map_err(|err| fail(err.to_string()))?;
    match response.status().as_u16() {
        200 => response
            .json::<serde_json::Value>()
            .map_err(|err| fail(err.to_string()))?
            .get("revision")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| fail(String::from("resposta sem revision"))),
        401 => Err(SharedCatalogError::Unauthorized),
        409 => {
            // `None` = sem vigente (criar com base sobre ausente) ou nó
            // antigo sem `details`; o snapshot releito mostra a verdade.
            let current = response
                .json::<serde_json::Value>()
                .ok()
                .and_then(|body| body.get("details")?.get("current").cloned())
                .and_then(|value| {
                    if value.is_null() {
                        None
                    } else {
                        value.as_u64()
                    }
                });
            Err(SharedCatalogError::Conflict { current })
        }
        410 => Err(SharedCatalogError::Tombstoned),
        status => Err(fail(format!("status inesperado: {status}"))),
    }
}

/// Eventos contíguos desde o cursor (sem token).
pub fn events_since(base_url: &str, since: u64) -> Result<Vec<Event>, SharedCatalogError> {
    events_since_with_token(base_url, since, None)
}

/// Como [`events_since`], com token opcional (T-840-03a).
pub fn events_since_with_token(
    base_url: &str,
    since: u64,
    token: Option<&str>,
) -> Result<Vec<Event>, SharedCatalogError> {
    let url = base_url.trim_end_matches('/');
    let fail = |detail: String| SharedCatalogError::Unreachable {
        url: String::from(url),
        detail,
    };
    let response = authorized(
        client(url)
            .map_err(|_| fail(String::from("cliente http")))?
            .get(format!("{url}/catalog/events?since={since}")),
        token,
    )
    .send()
    .map_err(|err| fail(err.to_string()))?;
    match response.status().as_u16() {
        200 => response
            .json::<Vec<Event>>()
            .map_err(|err| fail(err.to_string())),
        401 => Err(SharedCatalogError::Unauthorized),
        410 => Err(SharedCatalogError::CursorExpired),
        status => Err(fail(format!("status inesperado: {status}"))),
    }
}

mod catalog_feed;
mod catalog_poll;
mod catalog_state;
mod catalog_sync;

pub(crate) use catalog_feed::{CatalogMsg, FollowOutcome};
pub use catalog_feed::{ConflictInfo, FeedEntry, FeedTag};
pub use catalog_state::SharedCatalog;
