//! Inferência no Studio: cliente do servidor llama compatível OpenAI.
//!
//! Temperatura e limite de saída atravessam o contrato (vão no corpo do
//! `POST /v1/chat/completions`), nunca ficam só no formulário (§SDD:545+).
//! Cliente bloqueante rodando em THREAD de trabalho (padrão `models.rs`/
//! `launch.rs`: thread + mpsc + `poll` por frame) — a thread de UI nunca
//! bloqueia: `send` pode levar até 120 s (REQ/T-820-19).

use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use thiserror::Error;

/// Papel de uma mensagem no contrato de chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Mensagem do contrato de chat. `ts_unix_ms` é o relógio LOCAL de
/// recebimento/criação (só GUI — `skip` no fio, o contrato OpenAI não muda).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip)]
    pub ts_unix_ms: u64,
}

/// Requisição de geração: modelo, mensagens e parâmetros que atravessam o fio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Top-p (nucleus sampling) — atravessa o contrato como os demais.
    pub top_p: f32,
}

/// Estatísticas reais de um turno assistente (tela 3.3): duração medida +
/// tokens do `usage` quando o servidor os retorna.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TurnStats {
    pub elapsed_ms: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

impl TurnStats {
    /// Tokens/s derivado (Some só quando há tokens de saída e Δt > 0).
    #[must_use]
    pub fn tokens_per_sec(&self) -> Option<f64> {
        let tokens = self.completion_tokens? as f64;
        let secs = self.elapsed_ms as f64 / 1000.0;
        (secs > 0.0).then(|| tokens / secs)
    }
}

/// `usage` da resposta do servidor (campos opcionais — nem todo build envia).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct UsageInfo {
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    #[serde(default)]
    pub completion_tokens: Option<u64>,
}

/// Modelo anunciado por `GET /v1/models` (campos usados pela GUI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
}

/// Erros da inferência (fronteira GUI ↔ servidor llama).
#[derive(Debug, Error)]
pub enum InferenceError {
    /// Servidor inalcançável ou resposta fora do contrato.
    #[error("falha na inferencia em {url}: {detail}")]
    Unreachable { url: String, detail: String },
    /// Contrato vazio: sem escolhas para exibir.
    #[error("resposta sem escolhas")]
    EmptyChoices,
}

pub(crate) fn client(url: &str) -> Result<reqwest::blocking::Client, InferenceError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|err| InferenceError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

pub(crate) fn send(
    url: &str,
    request: reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::Response, InferenceError> {
    request
        .send()
        .map_err(|err| InferenceError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .error_for_status()
        .map_err(|err| InferenceError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

mod inference_api;
mod inference_state;

pub use inference_api::{chat_completion, chat_completion_with_stats, list_models};
pub use inference_state::InferenceState;
