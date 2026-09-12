//! Workflows (T10/UI-4.1): catálogo dos workloads com dependências
//! fiéis (sequência, fork-join, serial). Clique emite UMA intenção;
//! redesenhar nunca reemite (UI-G26).

/// Tipo de workload com semântica de dependência canônica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadKind {
    Sequence,
    ForkJoin,
    Serial,
}

impl WorkloadKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sequence => "sequência",
            Self::ForkJoin => "fork-join",
            Self::Serial => "serial",
        }
    }

    /// Dependências em linguagem explícita (nunca DAG genérico).
    #[must_use]
    pub const fn dependencies(self) -> &'static str {
        match self {
            Self::Sequence => "B depende de A",
            Self::ForkJoin => "A e B independentes; C depende de ambos",
            Self::Serial => "mesmo trabalho independente, sem sobreposição",
        }
    }
}

/// Solicitação completa a encaminhar ao serviço/runner existente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRequest {
    pub workflow_id: String,
    pub kind: WorkloadKind,
    pub profile: String,
    pub destination: String,
}

/// Intenção registrada pelo clique (efeito só via adapter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowIntent {
    pub request: WorkflowRequest,
    pub gesture: u64,
}

/// Catálogo + log de intenções (deduplicado por gesto).
#[derive(Debug, Clone, Default)]
pub struct WorkflowPanel {
    pub requests: Vec<WorkflowRequest>,
    pub intents: Vec<WorkflowIntent>,
    next_gesture: u64,
}

impl WorkflowPanel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra UMA intenção por gesto; repetir o mesmo gesto não duplica.
    pub fn submit(&mut self, request: WorkflowRequest) {
        self.next_gesture += 1;
        let gesture = self.next_gesture;
        if self.intents.iter().any(|intent| intent.gesture == gesture) {
            return;
        }
        self.intents.push(WorkflowIntent { request, gesture });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> WorkflowRequest {
        WorkflowRequest {
            workflow_id: String::from("wf-1"),
            kind: WorkloadKind::ForkJoin,
            profile: String::from("Balanced"),
            destination: String::from("agents-01"),
        }
    }

    #[test]
    fn fork_join_dependencies_are_explicit() {
        assert_eq!(
            WorkloadKind::ForkJoin.dependencies(),
            "A e B independentes; C depende de ambos"
        );
        assert_eq!(WorkloadKind::Sequence.dependencies(), "B depende de A");
    }

    #[test]
    fn each_submit_registers_single_intent() {
        let mut panel = WorkflowPanel::new();
        panel.submit(request());
        panel.submit(request());
        assert_eq!(panel.intents.len(), 2);
        assert_ne!(panel.intents[0].gesture, panel.intents[1].gesture);
    }
}
