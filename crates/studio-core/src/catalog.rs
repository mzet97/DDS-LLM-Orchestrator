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
mod tests {
    use super::*;
    use crate::revision::Generation;

    fn id(name: &str) -> DefinitionId {
        DefinitionId(name.into())
    }

    #[test]
    fn stale_update_is_rejected_before_any_effect() {
        // Given: definição publicada na rev 0 (G-47).
        let mut catalog = Catalog::new();
        let rev = catalog
            .publish(id("agent-a"), None, "v1".into(), Generation(1))
            .unwrap();
        assert_eq!(rev, Revision(0));
        // When: segunda GUI tenta atualizar da base obsoleta... (rev 0 após
        // a primeira GUI já ter avançado para rev 1). Atualização aceita
        // carrega geração estritamente maior (REQ-801, T-830-03).
        catalog
            .publish(id("agent-a"), Some(Revision(0)), "v2".into(), Generation(2))
            .unwrap();
        let err = catalog
            .publish(
                id("agent-a"),
                Some(Revision(0)),
                "rival".into(),
                Generation(2),
            )
            .unwrap_err();
        // Then: conflito com a vigente e valor preservado.
        assert_eq!(
            err,
            CatalogError::Conflict {
                current: Some(Revision(1))
            }
        );
        let snap = catalog.snapshot();
        assert_eq!(snap.items[0].value, "v2");
    }

    #[test]
    fn generation_regression_is_rejected_with_state_preserved() {
        // Given: item vigente com geração 2 (semente 1 + atualização 2).
        let mut catalog = Catalog::new();
        catalog
            .publish(id("dep"), None, "v1".into(), Generation(1))
            .unwrap();
        catalog
            .publish(id("dep"), Some(Revision(0)), "v2".into(), Generation(2))
            .unwrap();
        // When: GUI antiga tenta reimpor geração 1, regressiva (REQ-801).
        let err = catalog
            .publish(id("dep"), Some(Revision(1)), "antigo".into(), Generation(1))
            .unwrap_err();
        // Then: `StaleGeneration` tipado; repetida também é rejeitada.
        assert_eq!(
            err,
            CatalogError::StaleGeneration {
                got: Generation(1),
                current: Generation(2)
            }
        );
        assert_eq!(
            catalog
                .publish(id("dep"), Some(Revision(1)), "v2".into(), Generation(2))
                .unwrap_err(),
            CatalogError::StaleGeneration {
                got: Generation(2),
                current: Generation(2)
            }
        );
        // Estado preservado: valor e revisão da última aceita.
        let snap = catalog.snapshot();
        assert_eq!(snap.items[0].value, "v2");
        assert_eq!(snap.items[0].revision, Revision(1));
    }

    #[test]
    fn monotonic_generation_is_accepted_and_advances_revision() {
        // Given/When: criação semeia a geração; atualizações só avançam.
        let mut catalog = Catalog::new();
        catalog
            .publish(id("dep"), None, "v1".into(), Generation(5))
            .unwrap();
        catalog
            .publish(id("dep"), Some(Revision(0)), "v2".into(), Generation(6))
            .unwrap();
        catalog
            .publish(id("dep"), Some(Revision(1)), "v3".into(), Generation(9))
            .unwrap();
        // Then: revisões avançam 0→1→2 e o valor vigente é o último.
        let snap = catalog.snapshot();
        assert_eq!(snap.items[0].value, "v3");
        assert_eq!(snap.items[0].revision, Revision(2));
    }

    #[test]
    fn tombstone_blocks_resurrection_and_forces_new_identity() {
        // Given: item publicado e excluído (G-57).
        let mut catalog = Catalog::new();
        catalog
            .publish(id("tool-x"), None, "v1".into(), Generation(1))
            .unwrap();
        catalog.delete(&id("tool-x"), Revision(0)).unwrap();
        // When/Then: republicar o mesmo id é rejeitado...
        assert_eq!(
            catalog
                .publish(id("tool-x"), None, "v1".into(), Generation(2))
                .unwrap_err(),
            CatalogError::Tombstoned {
                deleted_at: Revision(1)
            }
        );
        // ...mas identidade nova com mesmo nome lógico é aceita.
        let rev = catalog
            .publish(id("tool-x#2"), None, "v1".into(), Generation(2))
            .unwrap();
        assert_eq!(rev, Revision(0));
    }

    #[test]
    fn snapshot_and_events_form_gapless_sequence() {
        // Given: snapshot inicial (G-49).
        let mut catalog = Catalog::new();
        catalog
            .publish(id("a"), None, "1".into(), Generation(1))
            .unwrap();
        let snap = catalog.snapshot();
        // When: nova publicação após o snapshot.
        catalog
            .publish(id("b"), None, "2".into(), Generation(1))
            .unwrap();
        // Then: acompanhamento entrega exatamente o evento novo, contíguo.
        let events = catalog.events_since(snap.cursor).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Created);
        assert_eq!(events[0].id, id("b"));
        assert_eq!(events[0].seq + 1, catalog.snapshot().cursor.0);
    }

    #[test]
    fn expired_cursor_forces_full_resync() {
        // Given: cursor antigo além da retenção (G-49).
        let mut catalog = Catalog::new();
        let old = catalog.snapshot().cursor;
        for i in 0..(RETENTION as u64 + 2) {
            catalog
                .publish(id(&format!("k{i}")), None, "v".into(), Generation(1))
                .unwrap();
        }
        // When/Then: acompanhar do cursor antigo exige ressincronização.
        assert_eq!(
            catalog.events_since(old).unwrap_err(),
            CatalogError::CursorExpired
        );
        // E o snapshot atual continua íntegro.
        assert_eq!(catalog.snapshot().items.len(), RETENTION + 2);
    }
}
