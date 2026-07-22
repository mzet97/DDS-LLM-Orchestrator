//! HTTP engine for llama-server inference via OpenAI-compatible API.

use crate::engine::{Chunk, Engine, EngineError, InferRequest};
use async_stream::stream;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub struct HttpEngine {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatMessageResponse>,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,
}

impl HttpEngine {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }
}

impl Engine for HttpEngine {
    fn infer_stream(
        &self,
        req: InferRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<Chunk, EngineError>> + Send>> {
        let client = self.client.clone();
        let url = format!("{}/v1/chat/completions", self.base_url);

        let messages: Vec<ChatMessage> =
            serde_json::from_str(&req.messages_json).unwrap_or_default();

        let body = ChatRequest {
            model: req.model_name.clone(),
            messages,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            stream: false,
        };

        Box::pin(stream! {
            let resp = client
                .post(&url)
                .json(&body)
                .timeout(std::time::Duration::from_millis(req.timeout_ms))
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        EngineError::Timeout(req.timeout_ms)
                    } else {
                        EngineError::InferenceFailed(e.to_string())
                    }
                })?;

            let chat_resp: ChatResponse = resp
                .json()
                .await
                .map_err(|e| EngineError::InferenceFailed(e.to_string()))?;

            let content = chat_resp
                .choices
                .first()
                .and_then(|c| c.message.as_ref())
                .and_then(|m| m.content.clone())
                .unwrap_or_default();

            let tokens_prompt = chat_resp.usage.as_ref().and_then(|u| u.prompt_tokens).unwrap_or(0);
            let tokens_completion = chat_resp.usage.as_ref().and_then(|u| u.completion_tokens).unwrap_or(0);

            yield Ok(Chunk {
                content,
                seq_num: 0,
                is_final: true,
                tokens_prompt,
                tokens_completion,
            });
        })
    }
}
