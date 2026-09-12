//! Modo de demonstração (UI-5.1): fixtures com IDs estáveis e nomes
//! fictícios, cenários selecionáveis da tabela §20. Tudo rotulado
//! "Demonstração · dados simulados"; nunca mistura cache com o real.
//! Sem rede, GPU, modelo, credenciais ou serviços externos.

/// Cenários mínimos selecionáveis (§20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewScenario {
    EmptyEnvironment,
    ExistingEnvironment,
    LoadingCatalog,
    PartialInventory,
    ExternalResource,
    StoppedInstance,
    InvalidForm,
    HostChange,
    RevisionConflict,
    DeletedResource,
    SlowOperation,
    UnconfirmedResult,
    IncrementalFlow,
    PermissionDenied,
    MissingBackend,
    HighVolume,
}

impl PreviewScenario {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EmptyEnvironment => "Ambiente vazio",
            Self::ExistingEnvironment => "Ambiente existente",
            Self::LoadingCatalog => "Catálogo carregando",
            Self::PartialInventory => "Inventário parcial",
            Self::ExternalResource => "Recurso externo",
            Self::StoppedInstance => "Instância parada",
            Self::InvalidForm => "Formulário inválido",
            Self::HostChange => "Mudança de host",
            Self::RevisionConflict => "Conflito de revisão",
            Self::DeletedResource => "Recurso excluído",
            Self::SlowOperation => "Operação demorada",
            Self::UnconfirmedResult => "Resultado sem confirmação",
            Self::IncrementalFlow => "Fluxo incremental",
            Self::PermissionDenied => "Permissão negada",
            Self::MissingBackend => "Backend inexistente",
            Self::HighVolume => "Grande volume",
        }
    }

    /// Evidência visual/interativa esperada do cenário.
    #[must_use]
    pub const fn expected(self) -> &'static str {
        match self {
            Self::EmptyEnvironment => "Onboarding com ação pertinente.",
            Self::ExistingEnvironment => "Recuperar definições sem recriar.",
            Self::LoadingCatalog => "Zero nunca como dado confirmado.",
            Self::PartialInventory => "Fonte, idade e campos desconhecidos.",
            Self::ExternalResource => "Somente leitura, sem administração presumida.",
            Self::StoppedInstance => "Definição preservada, sem alarme falso.",
            Self::InvalidForm => "Correção local sem perda de valores.",
            Self::HostChange => "Revisão dos campos dependentes.",
            Self::RevisionConflict => "Rascunho preservado e comparação.",
            Self::DeletedResource => "Contexto preservado, sem ressurreição.",
            Self::SlowOperation => "Interface utilizável e acompanhamento.",
            Self::UnconfirmedResult => "Nenhum reenvio automático.",
            Self::IncrementalFlow => "Scroll estável com texto incremental.",
            Self::PermissionDenied => "Mensagem e ação compatíveis.",
            Self::MissingBackend => "Capacidade ausente explicada.",
            Self::HighVolume => "Limites e virtualização visíveis.",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::EmptyEnvironment,
            Self::ExistingEnvironment,
            Self::LoadingCatalog,
            Self::PartialInventory,
            Self::ExternalResource,
            Self::StoppedInstance,
            Self::InvalidForm,
            Self::HostChange,
            Self::RevisionConflict,
            Self::DeletedResource,
            Self::SlowOperation,
            Self::UnconfirmedResult,
            Self::IncrementalFlow,
            Self::PermissionDenied,
            Self::MissingBackend,
            Self::HighVolume,
        ]
    }
}

/// Recurso fictício com ID estável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureResource {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub state: String,
}

/// Conjunto de fixtures (§20: nó de controle, nó de agentes, dois
/// hosts de inferência e um gateway; modelos só por metadados).
#[derive(Debug, Clone)]
pub struct FixtureSet {
    pub resources: Vec<FixtureResource>,
}

impl FixtureSet {
    #[must_use]
    pub fn standard() -> Self {
        let resource = |id: &str, name: &str, kind: &str, state: &str| FixtureResource {
            id: String::from(id),
            name: String::from(name),
            kind: String::from(kind),
            state: String::from(state),
        };
        Self {
            resources: vec![
                resource(
                    "no-controle-01",
                    "controle-demo",
                    "nó de controle",
                    "gerenciado",
                ),
                resource(
                    "no-agentes-01",
                    "agentes-demo",
                    "nó de agentes",
                    "gerenciado",
                ),
                resource(
                    "inf-a-01",
                    "inferencia-a-demo",
                    "host de inferência",
                    "parcial",
                ),
                resource(
                    "inf-b-01",
                    "inferencia-b-demo",
                    "host de inferência",
                    "inacessível",
                ),
                resource("gw-01", "gateway-demo", "gateway", "somente leitura"),
                resource(
                    "model-a",
                    "modelo-a-demo",
                    "modelo (metadados)",
                    "configurado",
                ),
            ],
        }
    }
}

/// Rótulo obrigatório de qualquer superfície de demonstração.
#[must_use]
pub const fn demo_banner() -> &'static str {
    "Demonstração · dados simulados. Nenhuma máquina será alterada."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_scenarios_have_expected_evidence() {
        assert_eq!(PreviewScenario::all().len(), 16);
        for scenario in PreviewScenario::all() {
            assert!(!scenario.label().is_empty());
            assert!(!scenario.expected().is_empty());
        }
    }

    #[test]
    fn fixtures_use_stable_ids_and_fictional_names() {
        let fixtures = FixtureSet::standard();
        assert_eq!(fixtures.resources.len(), 6);
        let ids: Vec<&str> = fixtures
            .resources
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len());
        for item in &fixtures.resources {
            assert!(item.name.ends_with("-demo"));
        }
    }

    #[test]
    fn banner_names_simulation_and_no_effect() {
        assert!(demo_banner().contains("simulados"));
        assert!(demo_banner().contains("Nenhuma máquina será alterada"));
    }
}
