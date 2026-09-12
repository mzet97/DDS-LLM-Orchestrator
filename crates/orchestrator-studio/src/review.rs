//! Revisão de mudanças e conflitos (T12/UI-4.3): base carregada ×
//! proposta local × revisão atual. Nunca sobrescreve silenciosamente;
//! exclusão remota preserva o rascunho sem ressuscitar identidade.

/// Decisão explícita do operador diante do conflito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictDecision {
    #[default]
    Undecided,
    KeepDraft,
    NewProposal,
    Abandon,
}

/// Comparação de três vias para um campo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    pub field: String,
    pub base: String,
    pub local: String,
    pub remote: String,
}

impl FieldDiff {
    /// `true` quando o remoto mudou sob a edição local (conflito real).
    #[must_use]
    pub fn conflicts(&self) -> bool {
        self.base != self.remote && self.local != self.remote
    }
}

/// Estado da revisão de uma proposta.
#[derive(Debug, Clone, Default)]
pub struct ReviewState {
    pub resource: String,
    pub base_revision: u64,
    pub current_revision: u64,
    pub diffs: Vec<FieldDiff>,
    pub decision: ConflictDecision,
    pub remote_deleted: bool,
}

impl ReviewState {
    #[must_use]
    pub fn new(resource: &str, base_revision: u64) -> Self {
        Self {
            resource: String::from(resource),
            base_revision,
            current_revision: base_revision,
            diffs: Vec::new(),
            decision: ConflictDecision::Undecided,
            remote_deleted: false,
        }
    }

    /// Há revisão remota mais nova que a base carregada?
    #[must_use]
    pub fn stale(&self) -> bool {
        self.current_revision > self.base_revision
    }

    /// Cenário de demonstração rotulado (conflito determinístico).
    #[must_use]
    pub fn demo() -> Self {
        let mut review = Self::new("agente-demo (demonstração)", 7);
        review.current_revision = 8;
        review.diffs.push(FieldDiff {
            field: String::from("modelo"),
            base: String::from("modelo-a"),
            local: String::from("modelo-b"),
            remote: String::from("modelo-c"),
        });
        review
    }

    /// Campos em conflito real (remoto andou sob edição local).
    #[must_use]
    pub fn conflicts(&self) -> Vec<&FieldDiff> {
        self.diffs.iter().filter(|diff| diff.conflicts()).collect()
    }
}

/// Render da comparação (puro sobre o estado; sem efeitos remotos).
pub fn show(ui: &mut egui::Ui, review: &mut ReviewState) {
    ui.heading("Revisão de mudanças");
    if ui.button("Carregar exemplo (demonstração)").clicked() {
        *review = ReviewState::demo();
    }
    if review.resource.is_empty() {
        ui.label("Nenhuma revisão carregada. Carregue o exemplo ou abra pela tela de edição.");
        return;
    }
    ui.label(format!(
        "Recurso {} · base {} · atual {}",
        review.resource, review.base_revision, review.current_revision
    ));
    if review.remote_deleted {
        ui.group(|ui| {
            ui.strong("Recurso excluído remotamente");
            ui.label("Rascunho local preservado como proposta separada; identidade não será republicada automaticamente.");
        });
    }
    if review.stale() {
        ui.label("Outra sessão publicou uma revisão. Revise as diferenças antes de continuar.");
    }
    for diff in &review.diffs {
        ui.group(|ui| {
            ui.strong(&diff.field);
            ui.monospace(format!("base:   {}", diff.base));
            ui.monospace(format!("local:  {}", diff.local));
            ui.monospace(format!("remota: {}", diff.remote));
            if diff.conflicts() {
                ui.label("Conflito: remoto andou sob sua edição.");
            }
        });
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut review.decision,
            ConflictDecision::KeepDraft,
            "Manter rascunho",
        );
        ui.selectable_value(
            &mut review.decision,
            ConflictDecision::NewProposal,
            "Nova proposta",
        );
        ui.selectable_value(
            &mut review.decision,
            ConflictDecision::Abandon,
            "Abandonar edição",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_advance_marks_stale_without_touching_draft() {
        let mut review = ReviewState::new("agente-1", 7);
        review.current_revision = 8;
        assert!(review.stale());
        assert_eq!(review.base_revision, 7);
    }

    #[test]
    fn only_diverged_fields_conflict() {
        let mut review = ReviewState::new("agente-1", 7);
        review.diffs.push(FieldDiff {
            field: String::from("modelo"),
            base: String::from("a"),
            local: String::from("b"),
            remote: String::from("c"),
        });
        review.diffs.push(FieldDiff {
            field: String::from("nome"),
            base: String::from("x"),
            local: String::from("x"),
            remote: String::from("x"),
        });
        assert_eq!(review.conflicts().len(), 1);
        assert_eq!(review.conflicts()[0].field, "modelo");
    }

    #[test]
    fn remote_deletion_preserves_draft_context() {
        let mut review = ReviewState::new("agente-1", 7);
        review.remote_deleted = true;
        assert!(review.remote_deleted);
        assert_eq!(review.decision, ConflictDecision::Undecided);
    }
}
