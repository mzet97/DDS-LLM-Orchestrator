//! Operações e logs (T11/UI-4.2): timeline por tipo de atividade,
//! visualizador com scroll estável e "Parar de acompanhar" quando não
//! há capacidade de cancelamento. Envio ≠ aceitação ≠ conclusão.

use std::collections::VecDeque;

/// Estado de confirmação da operação (dimensão "Operação", §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Prepared,
    Sent,
    Accepted,
    Running,
    Done,
    Failed,
    Unconfirmed,
}

impl OpState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prepared => "preparada",
            Self::Sent => "enviada",
            Self::Accepted => "aceita",
            Self::Running => "em andamento",
            Self::Done => "concluída",
            Self::Failed => "falhou",
            Self::Unconfirmed => "sem confirmação",
        }
    }
}

/// Evento da timeline (terminal preservado mesmo sob descarte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpEvent {
    pub detail: String,
    pub terminal: bool,
}

/// Registro de operação acompanhada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecord {
    pub id: String,
    pub kind: String,
    pub resource: String,
    pub machine: String,
    pub state: OpState,
    pub events: Vec<OpEvent>,
    pub cancel_supported: bool,
}

impl OperationRecord {
    /// Ação honesta: cancela só com capacidade; senão, parar de acompanhar.
    #[must_use]
    pub const fn follow_action(self_state: OpState, cancel_supported: bool) -> &'static str {
        match (self_state, cancel_supported) {
            (OpState::Done | OpState::Failed, _) => "arquivada",
            (_, true) => "Cancelar execução",
            (_, false) => "Parar de acompanhar",
        }
    }
}

/// Visualizador de log: buffer limitado, pausa, pesquisa e acompanhamento.
#[derive(Debug, Clone)]
pub struct LogViewer {
    lines: VecDeque<String>,
    pub capacity: usize,
    pub follow: bool,
    pub query: String,
}

impl LogViewer {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            capacity: capacity.max(1),
            follow: true,
            query: String::new(),
        }
    }

    /// Acrescenta em lote; excedente descarta as mais antigas.
    pub fn push_batch(&mut self, batch: &[String]) {
        for line in batch {
            if self.lines.len() >= self.capacity {
                self.lines.pop_front();
            }
            self.lines.push_back(line.clone());
        }
    }

    /// Linhas visíveis sob o filtro atual (pesquisa não perde posição).
    #[must_use]
    pub fn visible(&self) -> Vec<&str> {
        self.lines
            .iter()
            .map(String::as_str)
            .filter(|line| self.query.is_empty() || line.contains(&self.query))
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Painel de acompanhamento: registros por tipo + log com limite.
#[derive(Debug, Clone)]
pub struct OperationsPanel {
    pub records: Vec<OperationRecord>,
    pub log: LogViewer,
    pub kind_filter: String,
}

impl OperationsPanel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            log: LogViewer::new(10_000),
            kind_filter: String::new(),
        }
    }

    /// Registra operação; filtro vazio mostra todas.
    pub fn track(&mut self, record: OperationRecord) {
        self.records.push(record);
    }

    /// Registros sob o filtro atual (tipo contém o texto).
    #[must_use]
    pub fn visible(&self) -> Vec<&OperationRecord> {
        self.records
            .iter()
            .filter(|record| self.kind_filter.is_empty() || record.kind.contains(&self.kind_filter))
            .collect()
    }
}

impl Default for OperationsPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_not_done_stays_running_wording() {
        assert_eq!(OpState::Accepted.label(), "aceita");
        assert_eq!(OpState::Unconfirmed.label(), "sem confirmação");
    }

    #[test]
    fn no_cancel_capacity_offers_stop_following() {
        assert_eq!(
            OperationRecord::follow_action(OpState::Running, false),
            "Parar de acompanhar"
        );
        assert_eq!(
            OperationRecord::follow_action(OpState::Running, true),
            "Cancelar execução"
        );
    }

    #[test]
    fn buffer_caps_oldest_lines_first() {
        let mut viewer = LogViewer::new(3);
        viewer.push_batch(&[
            String::from("l1"),
            String::from("l2"),
            String::from("l3"),
            String::from("l4"),
        ]);
        assert_eq!(viewer.len(), 3);
        assert_eq!(viewer.visible(), vec!["l2", "l3", "l4"]);
    }

    #[test]
    fn kind_filter_separates_activity_types() {
        let mut panel = OperationsPanel::new();
        panel.track(OperationRecord {
            id: String::from("op-1"),
            kind: String::from("implantação de servidor"),
            resource: String::from("srv-1"),
            machine: String::from("gpu-a"),
            state: OpState::Running,
            events: Vec::new(),
            cancel_supported: false,
        });
        panel.track(OperationRecord {
            id: String::from("op-2"),
            kind: String::from("tarefa de inferência"),
            resource: String::from("task-1"),
            machine: String::from("agents-01"),
            state: OpState::Done,
            events: Vec::new(),
            cancel_supported: false,
        });
        assert_eq!(panel.visible().len(), 2);
        panel.kind_filter = String::from("inferência");
        assert_eq!(panel.visible().len(), 1);
        assert_eq!(panel.visible()[0].id, "op-2");
    }

    #[test]
    fn query_filters_without_losing_lines() {
        let mut viewer = LogViewer::new(10);
        viewer.push_batch(&[String::from("ok 1"), String::from("erro x")]);
        viewer.query = String::from("erro");
        assert_eq!(viewer.visible(), vec!["erro x"]);
        viewer.query.clear();
        assert_eq!(viewer.len(), 2);
    }
}
