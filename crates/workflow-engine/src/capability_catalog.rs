//! Catálogo de capacidades semânticas versionado (M2-02/M3-01).
//!
//! Distinto de `ModelSpecialization` (eixo modelo): capacidades são CLASSES
//! de trabalho (Prompt Master §3). O catálogo é a autoridade de rótulos
//! válidos para `Routing.Decision.capability_probabilities`,
//! `Agent.Capabilities.capabilities` e `PlanNode.required_capabilities`.
//! Versão 1 = mesma lista do worker Python (`questions.py::CAPABILITY_CATALOG`)
//! — paridade travada por teste no `tests/catalog_parity.rs`.

/// Catálogo imutável por revisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Catalog {
    pub revision: u32,
    pub labels: &'static [&'static str],
}

/// Revisão 1 (fase 900). NÃO reordenar/renomear sem nova revisão: os rótulos
/// entram em TypeObjects? Não — entram no wire como strings dos tópicos
/// novos; a revisão é fixada por intenção (`capability_catalog_revision`).
pub const V1: Catalog = Catalog {
    revision: 1,
    labels: &[
        "rust_refactor",
        "python_refactor",
        "unit_testing",
        "security_audit",
        "code_review",
        "research",
        "summarization",
        "translation",
    ],
};

/// Catálogo ativo (hoje: V1).
pub const fn catalog_v1() -> Catalog {
    V1
}

impl Catalog {
    pub fn contains(&self, label: &str) -> bool {
        self.labels.contains(&label)
    }

    /// Rótulos desconhecidos de um conjunto (para rejeição tipada).
    pub fn unknown<'a, I: IntoIterator<Item = &'a str>>(&self, labels: I) -> Vec<&'a str> {
        labels.into_iter().filter(|l| !self.contains(l)).collect()
    }

    /// Rótulos duplicados (para rejeição de distribuição com duplicatas).
    pub fn duplicates<'a, I: IntoIterator<Item = &'a str>>(labels: I) -> Vec<&'a str> {
        let mut seen = std::collections::HashSet::new();
        let mut dups = Vec::new();
        for l in labels {
            if !seen.insert(l) {
                dups.push(l);
            }
        }
        dups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_has_eight_labels_no_duplicates() {
        assert_eq!(V1.revision, 1);
        assert_eq!(V1.labels.len(), 8);
        assert!(Catalog::duplicates(V1.labels.iter().copied()).is_empty());
    }

    #[test]
    fn membership_and_unknown() {
        assert!(V1.contains("rust_refactor"));
        assert!(!V1.contains("Rust_refactor")); // case-sensitive: rótulo é id
        assert_eq!(V1.unknown(["rust_refactor", "magic"]), vec!["magic"]);
    }
}
