//! Claim loop e seleção de tasks (REQ-201, REQ-207, T-202, T-203).
//!
//! - Assina Tasks via DDS (stream async)
//! - Filtra por especialização e target_agent
//! - Claim: escreve ASSIGNED com assigned_agent
//! - Confirma ownership via readback

use dds_contract::generated::dds_llm_orchestrator::Task;
use std::collections::HashSet;

/// Especialização do agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Specialization {
    Text,
    Vision,
    Embedding,
    Transcription,
}

impl Specialization {
    /// Verifica se esta especialização pode atender a task requerida.
    pub fn matches(&self, required: i32) -> bool {
        match self {
            Self::Text => true,                             // TEXT aceita qualquer coisa
            Self::Vision => required == 0 || required == 1, // TEXT ou VISION
            Self::Embedding => required == 2,
            Self::Transcription => required == 3,
        }
    }
}

/// Configuração do agente para claim.
#[derive(Debug, Clone)]
pub struct ClaimConfig {
    pub agent_id: String,
    pub specialization: Specialization,
    pub target_agent_prefix: String, // vazio = aceita qualquer target
}

/// Verifica se uma task é elegível para claim.
pub fn is_eligible(task: &Task, config: &ClaimConfig, claimed: &HashSet<String>) -> bool {
    // Já claimed por outro?
    if claimed.contains(&task.task_id) {
        return false;
    }

    // Só PENDING
    if task.status != 0 {
        return false;
    }

    // Especialização compatível?
    if !config.specialization.matches(task.model_required) {
        return false;
    }

    // target_agent compatível?
    if !config.target_agent_prefix.is_empty()
        && !task.target_agent.is_empty()
        && !task.target_agent.starts_with(&config.target_agent_prefix)
    {
        return false;
    }

    true
}

/// Cria uma task ASSIGNED a partir de uma PENDING.
pub fn claim_task(task: &Task, agent_id: &str) -> Task {
    let mut claimed = task.clone();
    claimed.assigned_agent = agent_id.to_string();
    claimed.status = 1; // ASSIGNED
    claimed.assigned_at_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    claimed
}

/// Confirma ownership via readback (REQ-203).
/// Retorna true se este agente ainda detém a task.
pub fn confirm_ownership(task: &Task, agent_id: &str) -> bool {
    task.assigned_agent == agent_id && task.status == 1 // ASSIGNED
}
