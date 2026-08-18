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
    pub fn new(base_url: &str) -> Result<Self, EngineError> {
        let parsed = reqwest::Url::parse(base_url)
            .map_err(|error| EngineError::InferenceFailed(error.to_string()))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| EngineError::InferenceFailed("llama URL sem host".into()))?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if !matches!(parsed.scheme(), "http" | "https")
            || !loopback
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(EngineError::InferenceFailed(
                "engine HTTP aceita somente URLs loopback http(s) sem credenciais".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| EngineError::InferenceFailed(error.to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn http_engine_accepts_only_loopback_urls() {
        assert!(HttpEngine::new("http://127.0.0.1:8082").is_ok());
        assert!(HttpEngine::new("https://[::1]:8082").is_ok());
        assert!(HttpEngine::new("https://api.example.com").is_err());
        assert!(HttpEngine::new("http://user:pass@localhost:8082").is_err());
    }

    #[tokio::test]
    async fn http_engine_does_not_follow_redirects() {
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_address = target.local_addr().unwrap();
        let redirect = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let redirect_address = redirect.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = redirect.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/secret\r\nContent-Length: 0\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let engine = HttpEngine::new(&format!("http://{redirect_address}")).unwrap();
        let mut stream = engine.infer_stream(InferRequest {
            request_id: "redirect".into(),
            model_name: "test".into(),
            messages_json: "[]".into(),
            temperature: 0.0,
            max_tokens: 1,
            stream: false,
            timeout_ms: 1_000,
        });

        assert!(stream.next().await.unwrap().is_err());
        server.await.unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
    }
}
