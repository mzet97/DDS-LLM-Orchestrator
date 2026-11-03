//! Observação DDS no Studio (P7/P9): leitura viva do domínio — agentes,
//! tool calls, métricas de sistema e presença de nós Studio (`Studio.
//! NodePresence`, T-890) — sem publicar nada (só `ownership 0`).
//!
//! Atrás da feature `dds`: sem ela, o Studio continua HTTP-only. Cada coleta
//! abre um `DataSpace` efêmero, drena o stream pela janela e fecha — sem
//! thread permanente e sem roubar amostras (`read`, nunca `take`).
//!
//! A coleta (`observe`, `block_on` de 1–30 s) roda em THREAD de trabalho
//! (padrão `models.rs`: thread + mpsc + `poll` por frame) — a thread de UI
//! nunca bloqueia (REQ/T-820-19).

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, DiscoveryEvent, StudioNodePresence, SystemMetric, Task, TaskOutput, ToolCallRequest,
};
use dds_dataspace::DataSpace;
use futures::StreamExt;
use thiserror::Error;

/// Cap de retenção de payloads íntegros (arguments/result/content) — o
/// suficiente para o inspetor da 3.13/3.11 sem inflar o snapshot.
const PAYLOAD_CAP: usize = 8 * 1024;

fn capped(text: &str) -> String {
    if text.len() <= PAYLOAD_CAP {
        text.to_owned()
    } else {
        let cut = text
            .char_indices()
            .nth(PAYLOAD_CAP)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        format!("{}…", &text[..cut])
    }
}

/// Linha de topologia exibida na GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRow {
    pub agent_id: String,
    pub model: String,
    pub slots_busy: u32,
    pub slots_total: u32,
}

/// Linha de tool call exibida na GUI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ToolRow {
    pub call_id: String,
    pub tool_name: String,
    pub status: i32,
    /// T-890-06: quem pediu a ferramenta (governança por requester).
    pub requester_id: String,
    /// Nível de segurança pedido (o policy-engine decide por ele).
    pub security_level: i32,
    /// Prévia do resultado truncada (a íntegra fica no fio).
    pub result_preview: String,
    /// Payload de chamada íntegro do fio (cap 8 KB) — inspetor 3.13.
    pub arguments_json: String,
    /// Resultado íntegro do fio (cap 8 KB) — inspetor 3.13.
    pub result_json: String,
    /// Duração real da chamada: `completed_at_ns − created_at_ns`
    /// (0 enquanto aberta — completed_at_ns = 0).
    pub duration_ms: u64,
}

/// Tarefa viva no tópico `Tasks` (aba "TAREFAS DDS" da 3.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub task_id: String,
    pub client_id: String,
    pub assigned_agent: String,
    pub model_name: String,
    /// Ciclo canônico: 0=PENDING, 1=ASSIGNED, 2=RUNNING, 3=DONE, 4=FAILED.
    pub status: i32,
    pub priority: i32,
    pub retry_count: u32,
    pub created_at_ns: u64,
    pub completed_at_ns: u64,
}

/// Chunk de saída no tópico `TaskOutput` (barra de log de tarefas da 3.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOutputRow {
    pub task_id: String,
    pub seq_num: u32,
    pub agent_id: String,
    pub is_final: bool,
    pub token_count: u32,
    pub emitted_at_ns: u64,
    /// Conteúdo íntegro (cap 8 KB) — prévia de 96 chars na tabela.
    pub content: String,
}

/// Métrica de sistema exibida na GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricRow {
    pub source: String,
    pub name: String,
    pub value: f64,
}

/// Evento de descoberta exibido na GUI (P3a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRow {
    pub event_type: String,
    pub topic_name: String,
    pub remote_entity: String,
}

/// Linha de nó Studio exibida na GUI (T-890 — 19º tópico
/// `Studio.NodePresence`; instalação viva no domínio).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioNodeRow {
    pub node_id: String,
    pub url: String,
    pub token_required: bool,
}

/// Erros da observação DDS.
#[derive(Debug, Error)]
pub enum ObserveError {
    /// Domínio inacessível (DDS fora, participante falhou, runtime falhou).
    #[error("falha ao observar o dominio {domain}: {detail}")]
    Unavailable { domain: u32, detail: String },
}

mod dds_rows;
mod dds_state;

pub use dds_rows::{
    agent_row, discovery_row, metric_row, security_level_label, status_label, studio_node_row,
    task_output_row, task_row, task_status_label, tool_row, DdsSnapshot,
};
pub(crate) use dds_rows::{dataspace, drain, runtime};
pub use dds_state::{observe, DdsState};

#[cfg(test)]
mod tests;
