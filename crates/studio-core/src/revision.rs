//! Guarda de revisão condicional do catálogo compartilhado (REQ-800/801).
//!
//! Semântica multi-GUI: publicar a partir de uma revisão obsoleta é rejeitado
//! antes de qualquer efeito — nunca last-writer-wins silencioso. Gerações de
//! intenção só avançam monotonicamente.

use thiserror::Error;

/// Revisão do catálogo compartilhado (REQ-800).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// Geração de intenção de um deployment (REQ-801).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(pub u64);

/// Erro de publicação condicional (REQ-800).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RevisionError {
    /// Base obsoleta: nada foi aplicado; `current` é a revisão vigente.
    #[error("revisao obsoleta: base {base:?}, vigente {current:?}")]
    Stale { base: Revision, current: Revision },
    /// Contador saturado: recusa em vez de voltar a zero.
    #[error("revisao saturada em {0:?}")]
    Overflow(Revision),
}

/// Erro de avanço de geração (REQ-801).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GenerationError {
    /// Geração antiga ou repetida: estado preservado.
    #[error("geracao obsoleta: proposta {proposed:?}, vigente {current:?}")]
    Stale {
        proposed: Generation,
        current: Generation,
    },
}

/// Guarda a revisão vigente do catálogo (REQ-800).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionGuard {
    current: Revision,
}

impl RevisionGuard {
    /// Cria a guarda a partir da revisão vigente conhecida.
    pub const fn new(current: Revision) -> Self {
        Self { current }
    }

    /// Revisão vigente.
    pub const fn current(self) -> Revision {
        self.current
    }

    /// Publica a partir de `base`: só aceita quando `base == current`, e
    /// nesse caso avança exatamente uma revisão (REQ-800).
    pub fn publish(&mut self, base: Revision) -> Result<Revision, RevisionError> {
        if base != self.current {
            return Err(RevisionError::Stale {
                base,
                current: self.current,
            });
        }
        let next = self
            .current
            .0
            .checked_add(1)
            .map(Revision)
            .ok_or(RevisionError::Overflow(self.current))?;
        self.current = next;
        Ok(next)
    }
}

/// Guarda a geração vigente de uma intenção de deployment (REQ-801).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationGuard {
    current: Generation,
}

impl GenerationGuard {
    /// Cria a guarda a partir da geração vigente conhecida.
    pub const fn new(current: Generation) -> Self {
        Self { current }
    }

    /// Geração vigente.
    pub const fn current(self) -> Generation {
        self.current
    }

    /// Aceita somente geração estritamente maior que a vigente (REQ-801).
    pub fn accept(&mut self, proposed: Generation) -> Result<(), GenerationError> {
        if proposed <= self.current {
            return Err(GenerationError::Stale {
                proposed,
                current: self.current,
            });
        }
        self.current = proposed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_accepts_current_base_and_advances_one() {
        // Given: revisão vigente 7.
        let mut guard = RevisionGuard::new(Revision(7));
        // When: publica a partir da base 7.
        let next = guard.publish(Revision(7)).unwrap();
        // Then: avança exatamente para 8 e a guarda acompanha.
        assert_eq!(next, Revision(8));
        assert_eq!(guard.current(), Revision(8));
    }

    #[test]
    fn publish_rejects_stale_base_without_mutation() {
        // Given: vigente 8, proposta parte da base obsoleta 7.
        let mut guard = RevisionGuard::new(Revision(8));
        // When: tenta publicar da base 7.
        let err = guard.publish(Revision(7)).unwrap_err();
        // Then: rejeita com a vigente e nada muda (sem last-writer-wins).
        assert_eq!(
            err,
            RevisionError::Stale {
                base: Revision(7),
                current: Revision(8)
            }
        );
        assert_eq!(guard.current(), Revision(8));
    }

    #[test]
    fn generation_accepts_only_strictly_greater() {
        // Given: geração vigente 3.
        let mut guard = GenerationGuard::new(Generation(3));
        // When/Then: repetida e menor são rejeitadas com estado preservado.
        assert_eq!(
            guard.accept(Generation(3)).unwrap_err(),
            GenerationError::Stale {
                proposed: Generation(3),
                current: Generation(3)
            }
        );
        assert_eq!(
            guard.accept(Generation(2)).unwrap_err(),
            GenerationError::Stale {
                proposed: Generation(2),
                current: Generation(3)
            }
        );
        assert_eq!(guard.current(), Generation(3));
        // When: geração maior chega.
        guard.accept(Generation(4)).unwrap();
        // Then: vigente avança para 4.
        assert_eq!(guard.current(), Generation(4));
    }
}
