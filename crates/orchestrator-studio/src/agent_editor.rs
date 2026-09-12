//! Editor de agente (T05/UI-3.1): assistente em 4 passos para criação,
//! mesma estrutura em página para edição. Voltar preserva campos;
//! revisão mostra destinos distintos antes de qualquer efeito.

use crate::agent_defs::AgentDefinition;

/// Passos do assistente de criação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorStep {
    #[default]
    Identity,
    Execution,
    Tools,
    Review,
}

impl EditorStep {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "1 · Identidade e instruções",
            Self::Execution => "2 · Execução",
            Self::Tools => "3 · Ferramentas",
            Self::Review => "4 · Revisão",
        }
    }

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Identity => Some(Self::Execution),
            Self::Execution => Some(Self::Tools),
            Self::Tools => Some(Self::Review),
            Self::Review => None,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Option<Self> {
        match self {
            Self::Identity => None,
            Self::Execution => Some(Self::Identity),
            Self::Tools => Some(Self::Execution),
            Self::Review => Some(Self::Tools),
        }
    }
}

/// Erro de validação local por passo (valores nunca apagados).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepError {
    pub step: EditorStep,
    pub message: String,
}

/// Estado do editor: rascunho + passo + erros locais.
#[derive(Debug, Clone)]
pub struct AgentEditor {
    pub draft: AgentDefinition,
    pub step: EditorStep,
    pub errors: Vec<StepError>,
}

impl AgentEditor {
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self {
            draft: AgentDefinition::draft(id),
            step: EditorStep::Identity,
            errors: Vec::new(),
        }
    }

    /// Valida o passo atual sem apagar valores.
    pub fn validate_current(&mut self) -> bool {
        self.errors.retain(|err| err.step != self.step);
        let message = match self.step {
            EditorStep::Identity => {
                if self.draft.name.trim().is_empty() {
                    Some("nome do agente é obrigatório")
                } else {
                    None
                }
            }
            EditorStep::Execution => None,
            EditorStep::Tools => None,
            EditorStep::Review => None,
        };
        if let Some(message) = message {
            self.errors.push(StepError {
                step: self.step,
                message: String::from(message),
            });
            return false;
        }
        true
    }

    /// Avança validando; voltar nunca valida nem apaga.
    pub fn next(&mut self) {
        if self.validate_current() {
            if let Some(step) = self.step.next() {
                self.step = step;
            }
        }
    }

    /// Volta preservando todos os campos.
    pub fn back(&mut self) {
        if let Some(step) = self.step.previous() {
            self.step = step;
        }
    }

    /// Resumo final de destinos distintos (J02) para a revisão.
    #[must_use]
    pub fn review(&self) -> String {
        self.draft.destination_summary()
    }
}

/// Render do assistente (puro sobre o estado; sem efeitos remotos).
pub fn show(ui: &mut egui::Ui, editor: &mut AgentEditor) {
    ui.heading("Novo agente");
    ui.horizontal(|ui| {
        for step in [
            EditorStep::Identity,
            EditorStep::Execution,
            EditorStep::Tools,
            EditorStep::Review,
        ] {
            // Navegação livre só para trás; avançar exige passar pela validação.
            let past = (step as u8) <= (editor.step as u8);
            ui.add_enabled_ui(past, |ui| {
                if ui
                    .selectable_label(editor.step == step, step.label())
                    .clicked()
                {
                    editor.step = step;
                }
            });
        }
    });
    ui.separator();
    match editor.step {
        EditorStep::Identity => {
            ui.label("nome:");
            ui.text_edit_singleline(&mut editor.draft.name);
            ui.label("especialização:");
            ui.text_edit_singleline(&mut editor.draft.specialization);
            ui.label("instruções:");
            ui.text_edit_multiline(&mut editor.draft.instructions);
        }
        EditorStep::Execution => {
            ui.label("máquina do processo do agente:");
            ui.text_edit_singleline(&mut editor.draft.machine);
            ui.label("servidor/modelo de inferência:");
            ui.text_edit_singleline(&mut editor.draft.server);
        }
        EditorStep::Tools => {
            ui.label("ferramentas (uma por linha: identificador):");
            let mut tools = editor.draft.tools.join("\n");
            if ui.text_edit_multiline(&mut tools).changed() {
                editor.draft.tools = tools
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(String::from)
                    .collect();
            }
            ui.label("Associar não comprova loop de tool-use disponível.");
        }
        EditorStep::Review => {
            ui.strong("Revisão antes de qualquer efeito:");
            ui.label(editor.review());
            ui.label("Salvar rascunho persiste localmente; publicar solicita aceite do projeto.");
            ui.horizontal(|ui| {
                // Sem backend de aceite: botões sem efeito remoto nesta frente.
                let _draft = ui.button("Salvar rascunho");
                let _publish = ui.button("Solicitar publicação");
            });
        }
    }
    for error in editor.errors.iter().filter(|err| err.step == editor.step) {
        ui.label(format!("Corrija: {}", error.message));
    }
    ui.separator();
    ui.horizontal(|ui| {
        if editor.step.previous().is_some() && ui.button("Voltar").clicked() {
            editor.back();
        }
        if let Some(next) = editor.step.next() {
            if ui
                .button(match next {
                    EditorStep::Review => "Revisar",
                    _ => "Avançar",
                })
                .clicked()
            {
                editor.next();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_preserves_fields_without_validating() {
        let mut editor = AgentEditor::new("def-1");
        editor.draft.name = String::from("revisor");
        editor.next();
        assert_eq!(editor.step, EditorStep::Execution);
        editor.draft.machine = String::from("agents-01");
        editor.back();
        assert_eq!(editor.step, EditorStep::Identity);
        assert_eq!(editor.draft.name, "revisor");
        assert_eq!(editor.draft.machine, "agents-01");
        assert!(editor.errors.is_empty());
    }

    #[test]
    fn empty_name_blocks_advance_without_erasing() {
        let mut editor = AgentEditor::new("def-1");
        editor.next();
        assert_eq!(editor.step, EditorStep::Identity);
        assert_eq!(editor.errors.len(), 1);
        assert_eq!(editor.draft.name, "");
    }

    #[test]
    fn full_walk_reaches_review_with_summary() {
        let mut editor = AgentEditor::new("def-1");
        editor.draft.name = String::from("revisor");
        editor.next();
        editor.draft.machine = String::from("agents-01");
        editor.draft.server = String::from("inferencia-a em gpu-amd-01");
        editor.next();
        editor.next();
        assert_eq!(editor.step, EditorStep::Review);
        assert!(editor.review().contains("agents-01"));
    }
}
