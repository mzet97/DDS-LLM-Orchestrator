//! Catálogo autoritativo em memória: definições versionadas, snapshot/cursor e
//! tombstones (ST-STATE-01/02; G-47/49/57; §34.6/34.8/34.9/34.12).
//!
//! Semântica: cada definição tem sua própria revisão; publicar a partir de
//! base obsoleta é rejeitado antes de qualquer efeito. Exclusão persiste um
//! tombstone — o mesmo id nunca ressuscita; recriação deliberada exige
//! identidade nova. Leitores pedem snapshot + cursor e acompanham eventos
//! contíguos; cursor fora da retenção exige ressincronização total.
//!
//! REQ-800/801 agora são aplicados AQUI via [`crate::revision`]:
//! [`RevisionGuard`] guarda a regra condicional de revisão
//! (`base == current`, avanço exatamente um) em `publish`/`delete`, e
//! [`GenerationGuard`] tornou `generation` obrigatório — atualizar um item
//! existente com geração regressiva ou repetida é rejeitado com
//! [`CatalogError::StaleGeneration`] (T-830-03).

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::revision::{Generation, GenerationGuard, Revision, RevisionGuard};

/// Identidade lógica de uma definição publicada (G-52: nunca fundir por nome).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefinitionId(pub String);

/// Cursor opaco de acompanhamento de eventos (§34.8: inclui escopo/epoch no
/// protocolo; aqui, sequência monotônica do log local).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cursor(pub u64);

/// Evento administrativo persistido no log do catálogo (§34.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Posição no log; `cursor` após este evento é `Cursor(seq + 1)`.
    pub seq: u64,
    pub kind: EventKind,
    pub id: DefinitionId,
    pub revision: Revision,
}

/// Natureza do evento (§34.8: criação, atualização, exclusão).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Created,
    Updated,
    Deleted,
}

/// Visão de um item no snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemView {
    pub id: DefinitionId,
    pub value: String,
    pub revision: Revision,
}

/// Snapshot consistente + cursor de continuação (§34.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub items: Vec<ItemView>,
    pub cursor: Cursor,
}

/// Erros do catálogo (G-47/49/57).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    /// Base obsoleta ou item ausente onde era exigido: nada aplicado.
    #[error("conflito")]
    Conflict { current: Option<Revision> },
    /// Geração de intenção regressiva ou repetida sobre item vigente:
    /// estado preservado (REQ-801; T-830-03).
    #[error("geração obsoleta")]
    StaleGeneration {
        got: Generation,
        current: Generation,
    },
    /// Id com tombstone vigente: recriar exige identidade nova (G-57).
    #[error("id removido")]
    Tombstoned { deleted_at: Revision },
    /// Cursor fora da retenção: refazer snapshot (G-49).
    #[error("cursor expirado")]
    CursorExpired,
}

/// Catálogo autoritativo em memória (intenção + visão agregada, §34.6).
/// Cada item carrega valor, revisão e a geração vigente de intenção (REQ-801).
#[derive(Debug, Default)]
pub struct Catalog {
    items: HashMap<DefinitionId, (String, Revision, Generation)>,
    tombstones: HashMap<DefinitionId, Revision>,
    log: VecDeque<Event>,
    next_seq: u64,
}

/// Eventos retidos para acompanhamento; além disso, cursor expira (G-49).
const RETENTION: usize = 64;

impl Catalog {
    /// Catálogo vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Publica definição: `base=None` cria (rev 0), `Some(r)` atualiza a
    /// partir da revisão `r`. Rejeita obsoleto e id com tombstone (G-47/57).
    /// Revisão via [`RevisionGuard`] (REQ-800) e geração via
    /// [`GenerationGuard`] (REQ-801): sobre item existente, `generation`
    /// tem de ser ESTRITAMENTE maior que a vigente — primeira publicação
    /// mantém o comportamento antigo (geração proposta vira a semente).
    pub fn publish(
        &mut self,
        id: DefinitionId,
        base: Option<Revision>,
        value: String,
        generation: Generation,
    ) -> Result<Revision, CatalogError> {
        if let Some(deleted_at) = self.tombstones.get(&id) {
            return Err(CatalogError::Tombstoned {
                deleted_at: *deleted_at,
            });
        }
        let next = match (self.items.get(&id), base) {
            // Criação: id inédito, revisão inicial 0.
            (None, None) => Revision(0),
            // Atualização: base igual à vigente e avanço de exatamente um
            // (REQ-800 via guard); geração estritamente maior (REQ-801).
            // A revisão é verificada antes: base obsoleta continua sendo
            // `Conflict`, como antes do T-830-03.
            (Some((_, current, existing_generation)), Some(base)) => {
                let mut revisions = RevisionGuard::new(*current);
                let next = revisions
                    .publish(base)
                    .map_err(|_| CatalogError::Conflict {
                        current: Some(*current),
                    })?;
                let mut generations = GenerationGuard::new(*existing_generation);
                generations
                    .accept(generation)
                    .map_err(|_| CatalogError::StaleGeneration {
                        got: generation,
                        current: *existing_generation,
                    })?;
                next
            }
            // Criação sobre id existente ou base divergente/ausente: conflito.
            (Some((_, current, _)), _) => {
                return Err(CatalogError::Conflict {
                    current: Some(*current),
                });
            }
            (None, Some(_)) => return Err(CatalogError::Conflict { current: None }),
        };
        let kind = if self.items.contains_key(&id) {
            EventKind::Updated
        } else {
            EventKind::Created
        };
        self.items.insert(id.clone(), (value, next, generation));
        self.push_event(kind, id, next);
        Ok(next)
    }

    /// Exclui: persiste tombstone com a revisão do ato (G-57, §34.12).
    /// A regra condicional de revisão passa pelo [`RevisionGuard`] (REQ-800,
    /// T-830-03): `base == current` e o ato avança exatamente uma revisão.
    pub fn delete(&mut self, id: &DefinitionId, base: Revision) -> Result<Revision, CatalogError> {
        let Some((_, current, _)) = self.items.get(id) else {
            return Err(CatalogError::Conflict { current: None });
        };
        let mut guard = RevisionGuard::new(*current);
        let at = guard.publish(base).map_err(|_| CatalogError::Conflict {
            current: Some(*current),
        })?;
        self.items.remove(id);
        self.tombstones.insert(id.clone(), at);
        self.push_event(EventKind::Deleted, id.clone(), at);
        Ok(at)
    }

    /// Snapshot consistente + cursor de continuação (§34.8).
    pub fn snapshot(&self) -> Snapshot {
        let mut items: Vec<ItemView> = self
            .items
            .iter()
            .map(|(id, (value, revision, _generation))| ItemView {
                id: id.clone(),
                value: value.clone(),
                revision: *revision,
            })
            .collect();
        items.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        Snapshot {
            items,
            cursor: Cursor(self.next_seq),
        }
    }

    /// Eventos contíguos a partir do cursor; expira fora da retenção (G-49).
    pub fn events_since(&self, cursor: Cursor) -> Result<Vec<Event>, CatalogError> {
        let oldest = self.next_seq.saturating_sub(self.log.len() as u64);
        if cursor.0 < oldest || cursor.0 > self.next_seq {
            return Err(CatalogError::CursorExpired);
        }
        Ok(self
            .log
            .iter()
            .filter(|event| event.seq >= cursor.0)
            .cloned()
            .collect())
    }

    /// Anexa evento ao log com retenção limitada (G-49).
    fn push_event(&mut self, kind: EventKind, id: DefinitionId, revision: Revision) {
        let event = Event {
            seq: self.next_seq,
            kind,
            id,
            revision,
        };
        self.next_seq += 1;
        self.log.push_back(event);
        while self.log.len() > RETENTION {
            self.log.pop_front();
        }
    }
}

#[cfg(test)]
mod tests;
