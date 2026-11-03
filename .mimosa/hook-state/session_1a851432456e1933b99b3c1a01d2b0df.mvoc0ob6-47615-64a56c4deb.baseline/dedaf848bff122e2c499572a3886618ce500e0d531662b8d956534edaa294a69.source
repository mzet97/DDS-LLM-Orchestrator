//! Log idempotente de operações com reconciliação (G-05/06, G-50).
//!
//! Mesmo `operation_id` repetido após perda de conexão retorna a operação
//! registrada sem criar nova instância; mesmo id com payload distinto é
//! recusado; serviço fora do escopo próprio é recusado (RF-05).

use std::collections::HashMap;
use std::path::Path;

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
    /// Falha ao persistir ou carregar o log em disco (P2).
    #[error("falha de persistencia: {0}")]
    Storage(String),
}

/// Retenção máxima do log (REQ/T-820-18): o log de operações vive no
/// processo inteiro — sem cap, `records`/`order` crescem sem limite. Guarda
/// as últimas `MAX_RECORDS` operações (FIFO por inserção); as mais antigas
/// são descartadas. Nota honesta: como a persistência é snapshot
/// (`save`/`load`), o que sai da memória também sai do snapshot —
/// reconciliação (`reconcile`) de ids mais antigos que a janela passa a
/// devolver `unknown_operation`, aceitável para um nó de ops de longa
/// duração.
const MAX_RECORDS: usize = 1000;

/// Log em memória das operações do nó, restrito aos serviços próprios.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OperationLog {
    owned_services: Vec<String>,
    records: HashMap<OperationId, OpRecord>,
    /// Ordem de inserção (define "último"; `default` lê DBs antigos sem ela).
    #[serde(default)]
    order: Vec<OperationId>,
}

impl OperationLog {
    /// Cria o log declarando os serviços próprios deste nó.
    #[must_use]
    pub fn new(owned_services: Vec<String>) -> Self {
        Self {
            owned_services,
            records: HashMap::new(),
            order: Vec::new(),
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
        self.records.insert(id.clone(), record.clone());
        self.order.push(id);
        // Retenção limitada (REQ/T-820-18): descarta as mais antigas em
        // ordem de inserção (FIFO) — `records` e `order` em consistência.
        while self.order.len() > MAX_RECORDS {
            let oldest = self.order.remove(0);
            self.records.remove(&oldest);
        }
        Ok(OpOutcome::Applied(record))
    }

    /// Reconciliação: relê a operação registrada após perda de conexão (G-06).
    #[must_use]
    pub fn reconcile(&self, id: &OperationId) -> Option<&OpRecord> {
        self.records.get(id)
    }

    /// Estado pretendido de um serviço: último `SetService` na ordem de
    /// inserção (`None` sem registro — inclui DBs antigos sem `order`).
    #[must_use]
    pub fn wanted(&self, service: &str) -> Option<bool> {
        self.order
            .iter()
            .rev()
            .find_map(|id| match &self.records.get(id)?.op {
                AdminOp::SetService {
                    service: name,
                    running,
                } if name == service => Some(*running),
                AdminOp::SetService { .. } | AdminOp::Bootstrap { .. } => None,
            })
    }

    /// Serviços próprios declarados.
    #[must_use]
    pub fn owned_services(&self) -> &[String] {
        &self.owned_services
    }

    /// Iterador sobre os registros (base do `GET /operations`).
    pub fn records(&self) -> impl Iterator<Item = &OpRecord> {
        self.records.values()
    }

    /// Ordem de inserção dos ids (T-890-05: persistência relacional preserva
    /// a ordem — `wanted` depende dela).
    #[must_use]
    pub fn order(&self) -> &[OperationId] {
        &self.order
    }

    /// Reconstrói o log a partir do estado persistido (T-890-05: backend
    /// relacional). A ordem dada é a ordem de inserção original; registros
    /// fora dela são rejeitados (nada é executado na carga — apenas estado).
    #[must_use]
    pub fn restore(owned_services: Vec<String>, records_in_order: Vec<OpRecord>) -> Self {
        let mut log = Self::new(owned_services);
        for record in records_in_order {
            if log.records.contains_key(&record.id) {
                continue; // id duplicado no meio persistido: mantém a 1ª ocorrência
            }
            let id = record.id.clone();
            log.records.insert(id.clone(), record);
            log.order.push(id);
        }
        while log.order.len() > MAX_RECORDS {
            let oldest = log.order.remove(0);
            log.records.remove(&oldest);
        }
        log
    }

    /// Persiste o log em JSON no caminho dado (P2: operações persistidas).
    pub fn save(&self, path: &Path) -> Result<(), NodeError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|err| NodeError::Storage(err.to_string()))?;
        std::fs::write(path, json).map_err(|err| NodeError::Storage(err.to_string()))
    }

    /// Carrega o log persistido; arquivo ausente ou corrompido é erro
    /// (primeiro boot usa `new`, nunca `load` cego).
    pub fn load(path: &Path) -> Result<Self, NodeError> {
        let json =
            std::fs::read_to_string(path).map_err(|err| NodeError::Storage(err.to_string()))?;
        serde_json::from_str(&json).map_err(|err| NodeError::Storage(err.to_string()))
    }
}

#[cfg(test)]
mod tests;
