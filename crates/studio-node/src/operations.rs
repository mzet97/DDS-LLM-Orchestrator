//! Log idempotente de operações com reconciliação (G-05/06, G-50).
//!
//! Mesmo `operation_id` repetido após perda de conexão retorna a operação
//! registrada sem criar nova instância; mesmo id com payload distinto é
//! recusado; serviço fora do escopo próprio é recusado (RF-05).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::{AdminOp, OperationId};

/// Registro de uma operação administrativa aceita pelo nó.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpRecord {
    pub id: OperationId,
    pub op: AdminOp,
}

/// Resultado de [`OperationLog::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpOutcome {
    /// Primeira vez que o id aparece: efeito registrado.
    Applied(OpRecord),
    /// Repetição idêntica: nenhum efeito novo, retorna o registro (G-06).
    AlreadyApplied(OpRecord),
}

/// Erros do log de operações.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NodeError {
    /// Mesmo id, payload distinto: recusado, nada aplicado (G-50).
    #[error("operation_id reutilizado com payload distinto")]
    OperationIdConflict,
    /// Serviço fora dos recursos próprios do nó (RF-05).
    #[error("servico fora do escopo do no: {0}")]
    OutOfScope(String),
}

/// Log em memória das operações do nó, restrito aos serviços próprios.
#[derive(Debug, Default)]
pub struct OperationLog {
    owned_services: Vec<String>,
    records: HashMap<OperationId, OpRecord>,
}

impl OperationLog {
    /// Cria o log declarando os serviços próprios deste nó.
    #[must_use]
    pub fn new(owned_services: Vec<String>) -> Self {
        Self {
            owned_services,
            records: HashMap::new(),
        }
    }

    /// Aplica a operação com idempotência por id + reconciliação por releitura.
    pub fn apply(&mut self, id: OperationId, op: AdminOp) -> Result<OpOutcome, NodeError> {
        match &op {
            AdminOp::SetService { service, .. }
                if !self.owned_services.iter().any(|own| own == service) =>
            {
                return Err(NodeError::OutOfScope(service.clone()));
            }
            AdminOp::SetService { .. } | AdminOp::Bootstrap { .. } => {}
        }
        if let Some(record) = self.records.get(&id) {
            if record.op == op {
                return Ok(OpOutcome::AlreadyApplied(record.clone()));
            }
            return Err(NodeError::OperationIdConflict);
        }
        let record = OpRecord { id: id.clone(), op };
        self.records.insert(id, record.clone());
        Ok(OpOutcome::Applied(record))
    }

    /// Reconciliação: relê a operação registrada após perda de conexão (G-06).
    #[must_use]
    pub fn reconcile(&self, id: &OperationId) -> Option<&OpRecord> {
        self.records.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_with(service: &str) -> OperationLog {
        OperationLog::new(vec![String::from(service)])
    }

    fn op_id(id: &str) -> OperationId {
        OperationId(String::from(id))
    }

    #[test]
    fn repeated_bootstrap_applies_once() {
        let mut log = OperationLog::new(Vec::new());
        let id = op_id("boot-1");
        let op = AdminOp::Bootstrap {
            node_name: String::from("no-a"),
        };

        let first = log.apply(id.clone(), op.clone());
        let second = log.apply(id.clone(), op.clone());

        assert!(matches!(first, Ok(OpOutcome::Applied(_))));
        assert!(matches!(second, Ok(OpOutcome::AlreadyApplied(_))));
        assert!(log.reconcile(&id).is_some());
    }

    #[test]
    fn same_id_with_distinct_payload_is_refused() {
        let mut log = log_with("dds-agent");
        let id = op_id("op-1");
        let running = AdminOp::SetService {
            service: String::from("dds-agent"),
            running: true,
        };
        let stopped = AdminOp::SetService {
            service: String::from("dds-agent"),
            running: false,
        };

        log.apply(id.clone(), running).expect("primeira aplica");
        let err = log
            .apply(id.clone(), stopped)
            .expect_err("payload distinto deve falhar");

        assert_eq!(err, NodeError::OperationIdConflict);
        let kept = log.reconcile(&id).expect("registro original preservado");
        assert_eq!(
            kept.op,
            AdminOp::SetService {
                service: String::from("dds-agent"),
                running: true,
            }
        );
    }

    #[test]
    fn service_outside_own_scope_is_refused() {
        let mut log = log_with("dds-agent");

        let err = log
            .apply(
                op_id("op-9"),
                AdminOp::SetService {
                    service: String::from("postgres-alheio"),
                    running: true,
                },
            )
            .expect_err("servico alheio deve falhar");

        assert_eq!(err, NodeError::OutOfScope(String::from("postgres-alheio")));
        assert!(log.reconcile(&op_id("op-9")).is_none());
    }

    #[test]
    fn reconcile_returns_none_for_unknown_id() {
        let log = log_with("dds-agent");

        assert!(log.reconcile(&op_id("inexistente")).is_none());
    }
}
