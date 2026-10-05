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

/// Resultado do acompanhamento de eventos (T-830-02), resolvido no worker.
enum FollowOutcome {
    /// Sem eventos: o cursor local já cobre o log.
    UpToDate,
    /// Só exclusões: o evento descreve tudo (id + revisão); a remoção é
    /// aplicada incrementalmente na tabela exibida.
    Deleted(Vec<Event>),
    /// Criação/atualização presente: o evento NÃO carrega o valor, então o
    /// lote termina em snapshot fresco (convergência honesta da tabela).
    Applied {
        count: usize,
        snapshot: Result<Snapshot, String>,
    },
    /// 410 `CursorExpired`: ressincronização total pelo snapshot.
    Resync(Result<Snapshot, String>),
    /// Falha de transporte: estado permanece; motivo vira aviso.
    Failed(String),
}

/// Mensagem do worker do catálogo.
enum CatalogMsg {
    /// Resposta de leitura do snapshot.
    Snapshot(Result<Snapshot, String>),
    /// Resposta de mutação + releitura do snapshot (mesma sequência do
    /// antigo `publish_form`/`delete_form`: muta, depois relê).
    Mutated {
        notice: String,
        then_snapshot: Option<Result<Snapshot, String>>,
    },
    /// Resultado do acompanhamento de eventos (T-830-02).
    Followed(FollowOutcome),
}

/// Estado do painel de catálogo compartilhado.
///
/// `token` (T-840-03a) vive só na memória da GUI — o `Debug` manual é
/// proposital: o valor NUNCA aparece em logs/derives (RNF-04).
#[derive(Default)]
pub struct SharedCatalog {
    pub url: String,
    pub snapshot: Option<Snapshot>,
    /// Cursor do log já incorporado à visão local (T-830-02): derivado do
    /// último snapshot aplicado ou do último lote só de exclusões.
    pub cursor: u64,
    pub form_id: String,
    pub form_value: String,
    pub form_base: String,
    pub notice: String,
    /// Token da autoridade (T-840-03a): vazio = sem token (modo localhost).
    pub token: String,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    receiver: Option<mpsc::Receiver<CatalogMsg>>,
}

impl std::fmt::Debug for SharedCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedCatalog")
            .field("url", &self.url)
            .field("snapshot", &self.snapshot)
            .field("cursor", &self.cursor)
            .field("form_id", &self.form_id)
            .field("form_value", &self.form_value)
            .field("form_base", &self.form_base)
            .field("notice", &self.notice)
            .field("token_definido", &!self.token.trim().is_empty())
            .field("busy", &self.busy)
            .field("receiver", &self.receiver.is_some())
            .finish()
    }
}

impl SharedCatalog {
    /// Padrão honesto: autoridade local, sem token.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            url: String::from(url),
            ..Self::default()
        }
    }

    /// Token efetivo da autoridade (campo de UI → `Option`).
    fn auth(&self) -> Option<&str> {
        optional_token(&self.token)
    }

    /// Relê o snapshot em background; erro vira aviso, nunca linhas
    /// inventadas. Clique durante `busy` é ignorado.
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let auth = self.auth().map(String::from);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(CatalogMsg::Snapshot(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("lendo snapshot…");
    }

    /// Publica o formulário (base vazia = criação) e relê em seguida — em
    /// background (REQ/T-820-19). Clique durante `busy` é ignorado.
    pub fn publish_form(&mut self) {
        if self.busy {
            return;
        }
        let base = self.form_base.trim().parse::<u64>().ok();
        let url = self.url.clone();
        let id = self.form_id.trim().to_string();
        let value = self.form_value.clone();
        let auth = self.auth().map(String::from);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let notice = match publish_with_token(&url, &id, base, &value, auth.as_deref()) {
                Ok(revision) => format!("publicado em r{revision}"),
                Err(err) => err.to_string(),
            };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(CatalogMsg::Mutated {
                notice,
                then_snapshot,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("publicando…");
    }

    /// Exclui o id do formulário com a base dada e relê em seguida — em
    /// background (REQ/T-820-19). Clique durante `busy` é ignorado.
    pub fn delete_form(&mut self) {
        if self.busy {
            return;
        }
        let base = self.form_base.trim().parse::<u64>().unwrap_or(u64::MAX);
        let url = self.url.clone();
        let id = self.form_id.trim().to_string();
        let auth = self.auth().map(String::from);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let notice = match delete_with_token(&url, &id, base, auth.as_deref()) {
                Ok(revision) => format!("removido em r{revision}"),
                Err(err) => err.to_string(),
            };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(CatalogMsg::Mutated {
                notice,
                then_snapshot,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("removendo…");
    }

    /// Acompanha eventos desde o cursor local (T-830-02) — em background
    /// (REQ/T-820-19). Clique durante `busy` é ignorado. Lote só de
    /// `Deleted` aplica a remoção incrementalmente; qualquer `Created`/
    /// `Updated` (evento não carrega valor) ou 410 `CursorExpired` termina
    /// em snapshot fresco. Nó inalcançável: estado permanece, motivo vira
    /// aviso com o mesmo texto do erro tipado.
    pub fn follow_events(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let auth = self.auth().map(String::from);
        let cursor = self.cursor;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = match events_since_with_token(&url, cursor, auth.as_deref()) {
                Ok(events) if events.is_empty() => FollowOutcome::UpToDate,
                Ok(events) if events.iter().all(|event| event.kind == EventKind::Deleted) => {
                    FollowOutcome::Deleted(events)
                }
                Ok(events) => FollowOutcome::Applied {
                    count: events.len(),
                    snapshot: fetch_snapshot_with_token(&url, auth.as_deref())
                        .map_err(|err| err.to_string()),
                },
                Err(SharedCatalogError::CursorExpired) => FollowOutcome::Resync(
                    fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
                ),
                Err(err) => FollowOutcome::Failed(err.to_string()),
            };
            let _ = tx.send(CatalogMsg::Followed(outcome));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = format!("acompanhando eventos desde o cursor {cursor}…");
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    /// Nota: dentro de `poll` o snapshot é aplicado por atribuição direta de
    /// campos (`cursor` + `snapshot`) — um método `&mut self` conflitaria com
    /// o empréstimo de `receiver` no mesmo escopo.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    CatalogMsg::Snapshot(result) => match result {
                        Ok(snapshot) => {
                            self.cursor = snapshot.cursor.0;
                            self.snapshot = Some(snapshot);
                            self.notice.clear();
                        }
                        Err(err) => {
                            self.notice = err;
                        }
                    },
                    CatalogMsg::Mutated {
                        notice,
                        then_snapshot,
                    } => {
                        self.notice = notice;
                        if let Some(result) = then_snapshot {
                            match result {
                                Ok(snapshot) => {
                                    self.cursor = snapshot.cursor.0;
                                    self.snapshot = Some(snapshot);
                                    self.notice.clear();
                                }
                                Err(err) => {
                                    self.notice = err;
                                }
                            }
                        }
                    }
                    CatalogMsg::Followed(outcome) => match outcome {
                        FollowOutcome::UpToDate => {
                            self.notice = format!("sem eventos novos (cursor {})", self.cursor);
                        }
                        FollowOutcome::Deleted(events) => {
                            let removed: Vec<DefinitionId> =
                                events.iter().map(|event| event.id.clone()).collect();
                            if let Some(snapshot) = &mut self.snapshot {
                                // Id excluído ganha tombstone no nó: nunca
                                // volta, então toda linha com o id sai.
                                snapshot.items.retain(|item| !removed.contains(&item.id));
                            }
                            if let Some(last) = events.last() {
                                self.cursor = last.seq + 1;
                            }
                            self.notice = format!(
                                "{} exclusão(ões) aplicada(s) (cursor {})",
                                events.len(),
                                self.cursor
                            );
                        }
                        FollowOutcome::Applied { count, snapshot } => match snapshot {
                            Ok(fresh) => {
                                let cursor = fresh.cursor.0;
                                self.snapshot = Some(fresh);
                                self.cursor = cursor;
                                self.notice = format!(
                                    "{count} novo(s) evento(s) aplicado(s) (cursor {cursor})"
                                );
                            }
                            Err(err) => {
                                self.notice = err;
                            }
                        },
                        FollowOutcome::Resync(snapshot) => match snapshot {
                            Ok(fresh) => {
                                let cursor = fresh.cursor.0;
                                self.snapshot = Some(fresh);
                                self.cursor = cursor;
                                self.notice = format!(
                                    "cursor expirado; snapshot reaplicado (cursor {cursor})"
                                );
                            }
                            Err(err) => {
                                self.notice = err;
                            }
                        },
                        FollowOutcome::Failed(err) => {
                            self.notice = err;
                        }
                    },
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
    }
}
