//! Definições de agentes (T04/UI-2.3): definição local versionada,
//! separada das instâncias observadas ao vivo.
//!
//! "Duplicar definição" cria rascunho com NOVA identidade; "Parar
//! instância" nunca apaga definição. Vínculo instância→definição só
//! existe quando associado explicitamente; senão, "Não confirmado".

use serde::{Deserialize, Serialize};

/// Definição de agente (rascunho local até publicação).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub specialization: String,
    pub instructions: String,
    pub machine: String,
    pub server: String,
    pub model: String,
    pub tools: Vec<String>,
    pub revision: u64,
    pub published: bool,
}

impl AgentDefinition {
    /// Rascunho vazio com identidade nova.
    #[must_use]
    pub fn draft(id: &str) -> Self {
        Self {
            id: String::from(id),
            name: String::new(),
            specialization: String::new(),
            instructions: String::new(),
            machine: String::new(),
            server: String::new(),
            model: String::new(),
            tools: Vec::new(),
            revision: 0,
            published: false,
        }
    }

    /// Resumo de destinos distintos (J02): máquina do processo,
    /// servidor/máquina de inferência e ferramentas.
    #[must_use]
    pub fn destination_summary(&self) -> String {
        format!(
            "Agente executará em: {}. Inferência solicitada ao servidor: {}. Ferramentas: {}.",
            if self.machine.is_empty() {
                "não escolhido"
            } else {
                &self.machine
            },
            if self.server.is_empty() {
                "não escolhido"
            } else {
                &self.server
            },
            if self.tools.is_empty() {
                "nenhuma".to_string()
            } else {
                self.tools.join(", ")
            }
        )
    }
}

/// Loja de definições + associações instância→definição.
#[derive(Debug, Clone, Default)]
pub struct DefinitionStore {
    definitions: Vec<AgentDefinition>,
    associations: Vec<(String, String)>,
    next_id: u64,
}

impl DefinitionStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cria rascunho com identidade nova e única.
    pub fn create_draft(&mut self) -> &AgentDefinition {
        self.next_id += 1;
        let id = format!("def-{}", self.next_id);
        self.definitions.push(AgentDefinition::draft(&id));
        self.definitions.last().expect("rascunho criado")
    }

    /// Duplica definição existente em rascunho com NOVA identidade.
    pub fn duplicate(&mut self, id: &str) -> Option<&AgentDefinition> {
        let source = self.definitions.iter().find(|item| item.id == id)?.clone();
        self.next_id += 1;
        let mut copy = source;
        copy.id = format!("def-{}", self.next_id);
        copy.revision = 0;
        copy.published = false;
        self.definitions.push(copy);
        self.definitions.last()
    }

    /// Remove definição (nunca remove instância: instâncias não vivem aqui).
    pub fn remove(&mut self, id: &str) {
        self.definitions.retain(|item| item.id != id);
        self.associations.retain(|(_, definition)| definition != id);
    }

    /// Associa instância observada a definição (vínculo comprovado).
    pub fn associate(&mut self, instance_id: &str, definition_id: &str) {
        self.associations
            .retain(|(instance, _)| instance != instance_id);
        self.associations
            .push((String::from(instance_id), String::from(definition_id)));
    }

    /// Definição da instância, ou `None` ("Não confirmado").
    #[must_use]
    pub fn definition_of(&self, instance_id: &str) -> Option<&AgentDefinition> {
        let definition_id = self
            .associations
            .iter()
            .find(|(instance, _)| instance == instance_id)
            .map(|(_, definition)| definition)?;
        self.definitions
            .iter()
            .find(|item| &item.id == definition_id)
    }

    #[must_use]
    pub fn list(&self) -> &[AgentDefinition] {
        &self.definitions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_mints_new_identity_as_unpublished_draft() {
        let mut store = DefinitionStore::new();
        let id = store.create_draft().id.clone();
        let copy = store.duplicate(&id).expect("duplica");
        assert_ne!(copy.id, id);
        assert!(!copy.published);
        assert_eq!(copy.revision, 0);
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn unassociated_instance_has_no_definition() {
        let store = DefinitionStore::new();
        assert_eq!(store.definition_of("inst-9"), None);
    }

    #[test]
    fn association_links_instance_to_definition() {
        let mut store = DefinitionStore::new();
        let id = store.create_draft().id.clone();
        store.associate("inst-1", &id);
        assert_eq!(
            store.definition_of("inst-1").map(|item| item.id.as_str()),
            Some(id.as_str())
        );
        store.remove(&id);
        assert_eq!(store.definition_of("inst-1"), None);
    }

    #[test]
    fn destination_summary_names_distinct_machines() {
        let mut definition = AgentDefinition::draft("def-1");
        definition.machine = String::from("agents-01");
        definition.server = String::from("inferencia-a em gpu-amd-01");
        let summary = definition.destination_summary();
        assert!(summary.contains("agents-01"));
        assert!(summary.contains("inferencia-a em gpu-amd-01"));
    }
}
