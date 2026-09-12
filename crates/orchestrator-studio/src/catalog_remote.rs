//! Catálogo compartilhado no Studio: cliente da autoridade do nó.
//!
//! Publicação/exclusão condicionais com base explícita; snapshot e eventos
//! para acompanhamento. Conflito nunca é silencioso: vira texto com a
//! revisão vigente e dispara releitura do snapshot.

use studio_core::catalog::{Event, Snapshot};
use thiserror::Error;

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

/// Snapshot consistente da autoridade.
pub fn fetch_snapshot(base_url: &str) -> Result<Snapshot, SharedCatalogError> {
    let url = base_url.trim_end_matches('/');
    let fail = |detail: String| SharedCatalogError::Unreachable {
        url: String::from(url),
        detail,
    };
    client(url)
        .map_err(|_| fail(String::from("cliente http")))?
        .get(format!("{url}/catalog/snapshot"))
        .send()
        .map_err(|err| fail(err.to_string()))?
        .error_for_status()
        .map_err(|err| fail(err.to_string()))?
        .json::<Snapshot>()
        .map_err(|err| fail(err.to_string()))
}

/// Publica condicionalmente; retorna a revisão resultante.
pub fn publish(
    base_url: &str,
    id: &str,
    base: Option<u64>,
    value: &str,
) -> Result<u64, SharedCatalogError> {
    mutate(base_url, "publish", id, base, Some(value))
}

/// Exclui com tombstone a partir da base.
pub fn delete(base_url: &str, id: &str, base: u64) -> Result<u64, SharedCatalogError> {
    mutate(base_url, "delete", id, Some(base), None)
}

fn mutate(
    base_url: &str,
    action: &str,
    id: &str,
    base: Option<u64>,
    value: Option<&str>,
) -> Result<u64, SharedCatalogError> {
    let url = base_url.trim_end_matches('/');
    let fail = |detail: String| SharedCatalogError::Unreachable {
        url: String::from(url),
        detail,
    };
    let mut body = serde_json::json!({"id": id, "base": base});
    if action == "publish" {
        body["value"] = serde_json::json!(value.unwrap_or_default());
        body["generation"] = serde_json::json!(0);
    }
    let response = client(url)
        .map_err(|_| fail(String::from("cliente http")))?
        .post(format!("{url}/catalog/{action}"))
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

/// Eventos contíguos desde o cursor.
pub fn events_since(base_url: &str, since: u64) -> Result<Vec<Event>, SharedCatalogError> {
    let url = base_url.trim_end_matches('/');
    let fail = |detail: String| SharedCatalogError::Unreachable {
        url: String::from(url),
        detail,
    };
    let response = client(url)
        .map_err(|_| fail(String::from("cliente http")))?
        .get(format!("{url}/catalog/events?since={since}"))
        .send()
        .map_err(|err| fail(err.to_string()))?;
    match response.status().as_u16() {
        200 => response
            .json::<Vec<Event>>()
            .map_err(|err| fail(err.to_string())),
        410 => Err(SharedCatalogError::CursorExpired),
        status => Err(fail(format!("status inesperado: {status}"))),
    }
}

/// Estado do painel de catálogo compartilhado.
#[derive(Debug, Clone, Default)]
pub struct SharedCatalog {
    pub url: String,
    pub snapshot: Option<Snapshot>,
    pub form_id: String,
    pub form_value: String,
    pub form_base: String,
    pub notice: String,
}

impl SharedCatalog {
    /// Padrão honesto: autoridade local.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            url: String::from(url),
            ..Self::default()
        }
    }

    /// Relê o snapshot; erro vira aviso, nunca linhas inventadas.
    pub fn refresh(&mut self) {
        match fetch_snapshot(&self.url.clone()) {
            Ok(snapshot) => {
                self.snapshot = Some(snapshot);
                self.notice.clear();
            }
            Err(err) => {
                self.notice = err.to_string();
            }
        }
    }

    /// Publica o formulário (base vazia = criação) e relê em seguida.
    pub fn publish_form(&mut self) {
        let base = self.form_base.trim().parse::<u64>().ok();
        let outcome = publish(
            &self.url.clone(),
            self.form_id.trim(),
            base,
            &self.form_value.clone(),
        );
        match outcome {
            Ok(revision) => {
                self.notice = format!("publicado em r{revision}");
                self.refresh();
            }
            Err(err) => {
                self.notice = err.to_string();
                self.refresh();
            }
        }
    }

    /// Exclui o id do formulário com a base dada e relê em seguida.
    pub fn delete_form(&mut self) {
        let base = self.form_base.trim().parse::<u64>().unwrap_or(u64::MAX);
        match delete(&self.url.clone(), self.form_id.trim(), base) {
            Ok(revision) => {
                self.notice = format!("removido em r{revision}");
                self.refresh();
            }
            Err(err) => {
                self.notice = err.to_string();
                self.refresh();
            }
        }
    }
}
