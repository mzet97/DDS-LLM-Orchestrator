//! Engine trait e MockEngine (REQ-203, T-201).
//!
//! O Engine abstrai a inferência LLM. O MockEngine emite chunks previsíveis
//! para testes sem dependência do llama-server.

use async_stream::stream;
use futures_core::Stream;
use std::pin::Pin;

/// Restrição de roteamento serializada no campo IDL `provider_constraint` (REQ-606).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ProviderConstraint {
    Any,
    #[default]
    LocalOnly,
    CloudOnly,
}

impl ProviderConstraint {
    pub const fn as_idl_literal(self) -> &'static str {
        match self {
            Self::Any => "ANY",
            Self::LocalOnly => "LOCAL_ONLY",
            Self::CloudOnly => "CLOUD_ONLY",
        }
    }
}

/// Chunk de saída do streaming.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: String,
    pub seq_num: u32,
    pub is_final: bool,
    pub tokens_prompt: u32,
    pub tokens_completion: u32,
}

/// Requisição de inferência.
#[derive(Debug, Clone)]
pub struct InferRequest {
    pub request_id: String,
    pub messages_json: String,
    pub model_name: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    pub timeout_ms: u64,
}

/// Trait abstrato para engines de inferência.
pub trait Engine: Send + Sync {
    /// Retorna um stream de chunks para a requisição.
    fn infer_stream(
        &self,
        req: InferRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<Chunk, EngineError>> + Send>>;
}

/// Erros do engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("timeout após {0}ms")]
    Timeout(u64),
    #[error("modelo não disponível: {0}")]
    ModelUnavailable(String),
    #[error("inferência falhou: {0}")]
    InferenceFailed(String),
    #[error("DDS error: {0}")]
    DdsError(String),
}

/// MockEngine para testes — emite chunks previsíveis.
pub struct MockEngine {
    pub chunk_content: String,
    pub num_chunks: u32,
    pub delay_ms: u64,
}

impl MockEngine {
    pub fn new(chunk_content: &str, num_chunks: u32, delay_ms: u64) -> Self {
        Self {
            chunk_content: chunk_content.to_string(),
            num_chunks,
            delay_ms,
        }
    }
}

impl Engine for MockEngine {
    fn infer_stream(
        &self,
        _req: InferRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<Chunk, EngineError>> + Send>> {
        let content = self.chunk_content.clone();
        let num_chunks = self.num_chunks;
        let delay_ms = self.delay_ms;

        Box::pin(stream! {
            for i in 0..num_chunks {
                if delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                let is_final = i == num_chunks - 1;
                yield Ok(Chunk {
                    content: format!("{content}-{i:04}"),
                    seq_num: i,
                    is_final,
                    tokens_prompt: 10,
                    tokens_completion: if is_final { num_chunks } else { 1 },
                });
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderConstraint;

    #[test]
    fn provider_constraint_uses_the_three_idl_literals() {
        assert_eq!(ProviderConstraint::Any.as_idl_literal(), "ANY");
        assert_eq!(ProviderConstraint::LocalOnly.as_idl_literal(), "LOCAL_ONLY");
        assert_eq!(ProviderConstraint::CloudOnly.as_idl_literal(), "CLOUD_ONLY");
    }

    #[test]
    fn provider_constraint_defaults_to_local_only() {
        assert_eq!(ProviderConstraint::default(), ProviderConstraint::LocalOnly);
    }
}
