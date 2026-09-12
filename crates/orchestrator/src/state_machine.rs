//! Task state machine (REQ-406, T-406).
//!
//! Transições válidas:
//! - PENDING → ASSIGNED
//! - ASSIGNED → RUNNING, PENDING (reassign), FAILED
//! - RUNNING → DONE, FAILED, PENDING (reassign)
//! - DONE, FAILED → terminal (sem transição)

use dds_contract::generated::dds_llm_orchestrator::Task;
use orch_common::FinishReason;

/// Status de task (espelha o enum IDL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    Pending = 0,
    Assigned = 1,
    Running = 2,
    Done = 3,
    Failed = 4,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed)
    }
}

impl TryFrom<i32> for TaskStatus {
    type Error = TransitionError;

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Assigned),
            2 => Ok(Self::Running),
            3 => Ok(Self::Done),
            4 => Ok(Self::Failed),
            _ => Err(TransitionError::UnknownStatus { value: v }),
        }
    }
}

/// Erro de transição: inválida ou com status desconhecido no wire.
#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("transição inválida: {from:?} → {to:?}")]
    Invalid { from: TaskStatus, to: TaskStatus },
    #[error("status desconhecido no wire: {value}")]
    UnknownStatus { value: i32 },
}

/// Verifica se a transição é válida.
pub fn can_transition(from: TaskStatus, to: TaskStatus) -> bool {
    matches!(
        (from, to),
        (TaskStatus::Pending, TaskStatus::Assigned)
            | (TaskStatus::Assigned, TaskStatus::Running)
            | (TaskStatus::Assigned, TaskStatus::Pending)  // reassign
            | (TaskStatus::Assigned, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Done)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Pending) // reassign
    )
}

/// Transição segura — retorna erro se inválida ou se o status no wire
/// for desconhecido (nunca assume `Pending` silencioso).
pub fn transition(task: &mut Task, to: TaskStatus) -> Result<(), TransitionError> {
    let from = TaskStatus::try_from(task.status)?;
    if !can_transition(from, to) {
        return Err(TransitionError::Invalid { from, to });
    }
    task.status = to as i32;
    Ok(())
}

/// Assign: PENDING → ASSIGNED.
pub fn assign(task: &mut Task, agent_id: &str) -> Result<(), TransitionError> {
    transition(task, TaskStatus::Assigned)?;
    task.assigned_agent = agent_id.to_string();
    task.assigned_at_ns = now_ns();
    Ok(())
}

/// Start running: ASSIGNED → RUNNING.
pub fn start_running(task: &mut Task) -> Result<(), TransitionError> {
    transition(task, TaskStatus::Running)?;
    task.started_at_ns = now_ns();
    Ok(())
}

/// Complete: RUNNING → DONE (motivo canônico do vocabulário `FinishReason`).
pub fn complete(task: &mut Task) -> Result<(), TransitionError> {
    transition(task, TaskStatus::Done)?;
    task.completed_at_ns = now_ns();
    task.finish_reason = FinishReason::Completion.as_str().to_string();
    Ok(())
}

/// Fail: ASSIGNED/RUNNING → FAILED.
pub fn fail(task: &mut Task, reason: &str) -> Result<(), TransitionError> {
    transition(task, TaskStatus::Failed)?;
    task.completed_at_ns = now_ns();
    task.finish_reason = reason.to_string();
    Ok(())
}

/// Reassign: ASSIGNED/RUNNING → PENDING (incrementa retry_count).
pub fn reassign(task: &mut Task, max_retries: u32) -> Result<bool, TransitionError> {
    if task.retry_count >= max_retries {
        fail(task, "MAX_RETRIES_EXCEEDED")?;
        return Ok(false);
    }
    transition(task, TaskStatus::Pending)?;
    task.retry_count += 1;
    task.assigned_agent.clear();
    task.assigned_at_ns = 0;
    task.started_at_ns = 0;
    Ok(true)
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(status: i32) -> Task {
        Task {
            status,
            ..Task::default()
        }
    }

    #[test]
    fn test_valid_transitions() {
        let mut t = make_task(0); // PENDING
        assert!(assign(&mut t, "agent-1").is_ok());
        assert_eq!(t.status, 1); // ASSIGNED

        assert!(start_running(&mut t).is_ok());
        assert_eq!(t.status, 2); // RUNNING

        assert!(complete(&mut t).is_ok());
        assert_eq!(t.status, 3); // DONE
    }

    #[test]
    fn test_invalid_transition_rejected() {
        let mut t = make_task(3); // DONE (terminal)
        assert!(transition(&mut t, TaskStatus::Pending).is_err());
    }

    #[test]
    fn test_complete_emits_canonical_finish_reason() {
        // Given: RUNNING / When: completa / Then: string canônica com ida-e-volta ao FR_*
        let mut t = make_task(2);
        complete(&mut t).unwrap();
        assert_eq!(t.finish_reason, "COMPLETION");
        assert_eq!(
            FinishReason::parse(&t.finish_reason),
            Some(FinishReason::Completion)
        );
        assert_eq!(i32::from(FinishReason::Completion), 1); // FR_COMPLETION
    }

    #[test]
    fn test_unknown_wire_status_rejected() {
        // Given: status fora do enum (wire corrompido)
        let mut t = make_task(99);
        // When: tenta qualquer transição / Then: rejeita sem tocar na task
        let err = transition(&mut t, TaskStatus::Assigned).unwrap_err();
        assert!(matches!(err, TransitionError::UnknownStatus { value: 99 }));
        assert_eq!(t.status, 99);
    }

    #[test]
    fn test_reassign_increments_retry() {
        let mut t = make_task(1); // ASSIGNED
        t.retry_count = 0;
        assert!(reassign(&mut t, 3).unwrap());
        assert_eq!(t.retry_count, 1);
        assert_eq!(t.status, 0); // PENDING
    }

    #[test]
    fn test_reassign_max_retries_fails() {
        let mut t = make_task(1); // ASSIGNED
        t.retry_count = 3;
        assert!(!reassign(&mut t, 3).unwrap());
        assert_eq!(t.status, 4); // FAILED
    }

    #[test]
    fn test_reclaim_after_reassign_full_cycle() {
        // Given: tentativa 1 estagnada em RUNNING (reaper detectou inatividade)
        let mut t = make_task(2);
        t.assigned_agent = "agent-1".into();
        t.retry_count = 0;
        // When: reaper devolve a PENDING e outro agente reivindica e conclui
        assert!(reassign(&mut t, 3).unwrap());
        assert_eq!(t.status, 0); // PENDING
        assert!(t.assigned_agent.is_empty()); // sem dono — tentativa antiga invalidada
        assert_eq!(t.retry_count, 1);
        assert!(assign(&mut t, "agent-2").is_ok());
        assert!(start_running(&mut t).is_ok());
        // Then: conclusão da tentativa 2 fecha o ciclo com motivo canônico
        assert!(complete(&mut t).is_ok());
        assert_eq!(t.status, 3); // DONE
        assert_eq!(t.finish_reason, "COMPLETION");
        // RESÍDUO DOCUMENTADO (§25.2 do entendimento): `TaskOutput` não carrega
        // tentativa/retry — outputs da tentativa 1 com mesmo (task_id, seq_num)
        // são indistinguíveis dos da tentativa 2 (sem fencing fim-a-fim).
    }
}
