//! Despacho no Studio: tarefa real via orquestrador (`POST
//! `/api/v1/chat/completions/sync`), que publica no DDS e aguarda o agente.
//!
//! Resposta crua do backend, sem enfeite: concluída (com agente, latência,
//! corpo bruto e headers) ou falha (com motivo). Temperatura/max_tokens vão
//! no corpo (o backend aceita, `orchestrator/src/http.rs`); timeout é do
//! cliente HTTP. O despacho roda em THREAD de trabalho (padrão `models.rs`:
//! thread com canal mpsc e `poll` por frame) — a thread de UI nunca bloqueia
//! (REQ/T-820-19).

use serde::Deserialize;
use std::sync::mpsc;
use thiserror::Error;

/// Decomposição da latência do `/sync` (campos reais T1–T6 do backend,
/// convertidos para ms): fila no agente × geração LLM × transporte ×
/// serialização — o card "TOTAL = FILA + GERAÇÃO + …" da tela 3.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LatencyBreakdown {
    pub queue_ms: u64,
    pub inference_ms: u64,
    pub transport_ms: u64,
    pub serial_ms: u64,
}

impl LatencyBreakdown {
    /// Constrói a partir dos seis campos ns do `/sync` (0 = ausente).
    #[must_use]
    pub fn from_ns(
        t_serialization_ns: u64,
        t_transport_send_ns: u64,
        t_agent_queue_ns: u64,
        t_inference_ns: u64,
        t_transport_return_ns: u64,
        t_deserialization_ns: u64,
    ) -> Self {
        Self {
            queue_ms: t_agent_queue_ns / 1_000_000,
            inference_ms: t_inference_ns / 1_000_000,
            transport_ms: (t_transport_send_ns + t_transport_return_ns) / 1_000_000,
            serial_ms: (t_serialization_ns + t_deserialization_ns) / 1_000_000,
        }
    }

    /// `true` se algum componente foi reportado (resposta antiga → None).
    #[must_use]
    pub fn is_meaningful(&self) -> bool {
        self.queue_ms > 0 || self.inference_ms > 0 || self.transport_ms > 0 || self.serial_ms > 0
    }
}

/// Resultado do despacho síncrono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Tarefa concluída por um agente.
    Completed {
        task_id: String,
        assigned_agent: Option<String>,
        latency_ms: u64,
        /// Conteúdo da resposta quando o backend o retorna.
        content: Option<String>,
        /// Decomposição real da latência (T1–T6) quando o backend reporta.
        breakdown: Option<LatencyBreakdown>,
        /// Status HTTP real da chamada `/sync` (200 quando conclui).
        http_status: u16,
        /// Corpo da resposta byte a byte (aba "Payload JSON Bruto").
        raw: String,
        /// Headers da resposta pré-formatados `k: v` (aba Headers).
        headers: String,
    },
    /// Tarefa falha com o motivo do backend.
    Failed { task_id: String, error: String },
}

/// Entrada do histórico de despachos da sessão (tela 3.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRecord {
    pub task_id: String,
    pub agent: Option<String>,
    pub latency_ms: Option<u64>,
    pub ok: bool,
    /// Prévia da resposta ou motivo da falha.
    pub detail: String,
    /// Decomposição fila × geração (colunas do histórico 3.6).
    pub queue_ms: Option<u64>,
    pub inference_ms: Option<u64>,
    /// Instante (ms unix) em que o desfecho chegou (coluna TIMESTAMP).
    pub timestamp_ms: u64,
    /// Modelo pedido (coluna MODELO ALVO + Re-enviar).
    pub model: String,
    /// Prompt pedido (Re-enviar repete o despacho; só RAM da sessão).
    pub prompt: String,
}

/// Erros do despacho (fronteira GUI ↔ orquestrador).
#[derive(Debug, Error)]
pub enum DispatchError {
    /// Orquestrador inalcançável, timeout ou fora do contrato.
    #[error("falha no despacho em {url}: {detail}")]
    Unreachable { url: String, detail: String },
}

/// Despacha o prompt e aguarda a conclusão (`sync`).
///
/// `temperature`/`max_tokens` vão no corpo (o backend valida: temp finita em
/// −2..=2, max 1..=8192 — `orchestrator/src/http.rs`); `timeout` limita o
/// cliente HTTP. Status, corpo bruto e headers da resposta viajam no
/// `Completed` para as abas da tela 3.6.
/// Mensagem do worker de despacho.
pub(crate) enum DispatchMsg {
    Done(Result<DispatchOutcome, String>),
}

/// Estado do painel de despacho.
#[derive(Debug)]
pub struct DispatchState {
    pub url: String,
    pub model: String,
    pub prompt: String,
    /// Temperatura como texto (padrão do backend: 0.7; enviada no corpo).
    pub temperature: String,
    /// Max tokens como texto (padrão do backend: 256; enviado no corpo).
    pub max_tokens: String,
    /// Timeout do cliente em ms como texto (era fixo em 300 s).
    pub timeout_ms: String,
    /// Aba do resultado: 0 formatada · 1 JSON bruto · 2 headers.
    pub selected_tab: u8,
    pub result: String,
    /// Último desfecho estruturado (card de resultado, tela 3.6).
    pub last: Option<DispatchOutcome>,
    /// Histórico dos despachos da SESSÃO (mais recente por último).
    pub history: Vec<DispatchRecord>,
    /// `true` enquanto o despacho roda em background (`poll` drena).
    pub busy: bool,
    /// Modelo/prompt do despacho em voo (o histórico registra o pedido).
    pending_model: String,
    pending_prompt: String,
    receiver: Option<mpsc::Receiver<DispatchMsg>>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self::new()
    }
}

mod workload_dispatch;
mod workload_state;

pub use workload_dispatch::dispatch_sync;

#[cfg(test)]
mod tests;
