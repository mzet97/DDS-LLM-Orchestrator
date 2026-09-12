//! Inferência no Studio: cliente do servidor llama compatível OpenAI.
//!
//! Temperatura e limite de saída atravessam o contrato (vão no corpo do
//! `POST /v1/chat/completions`), nunca ficam só no formulário (§SDD:545+).
//! Cliente bloqueante como `origin` (thread de UI sem runtime).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Papel de uma mensagem no contrato de chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Mensagem do contrato de chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Requisição de geração: modelo, mensagens e parâmetros que atravessam o fio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
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

fn client(url: &str) -> Result<reqwest::blocking::Client, InferenceError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|err| InferenceError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

fn send(
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

/// Estado do painel de inferência: parâmetros que atravessam o contrato,
/// prompt editável e última resposta (ou erro formatado, nunca vazio mudo).
#[derive(Debug, Clone)]
pub struct InferenceState {
    pub server_url: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub prompt: String,
    pub reply: String,
    pub models: Vec<String>,
    pub history: Vec<Message>,
}

impl InferenceState {
    /// Padrões honestos: servidor local, temperatura 0.2, 256 tokens.
    #[must_use]
    pub fn new() -> Self {
        Self {
            server_url: String::from("http://127.0.0.1:8082"),
            model: String::new(),
            temperature: 0.2,
            max_tokens: 256,
            prompt: String::new(),
            reply: String::new(),
            models: Vec::new(),
            history: Vec::new(),
        }
    }

    /// Atualiza a lista de modelos; erro vira texto no painel, não panic.
    pub fn refresh_models(&mut self) {
        match list_models(&self.server_url.clone()) {
            Ok(models) => {
                if let Some(first) = models.first() {
                    if self.model.is_empty() {
                        self.model = first.id.clone();
                    }
                }
                self.models = models.into_iter().map(|info| info.id).collect();
            }
            Err(err) => {
                self.reply = format!("erro ao listar modelos: {err}");
            }
        }
    }

    /// Geração real contra o servidor com o histórico da sessão no fio;
    /// resposta ou erro ficam no painel. Bloqueia a thread de UI
    /// (localhost; async quando justificar).
    pub fn send(&mut self) {
        self.history.push(Message {
            role: Role::User,
            content: self.prompt.clone(),
        });
        let chat = ChatRequest {
            model: self.model.clone(),
            messages: self.history.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };
        match chat_completion(&self.server_url.clone(), &chat) {
            Ok(content) => {
                self.history.push(Message {
                    role: Role::Assistant,
                    content: content.clone(),
                });
                self.reply = content;
                self.prompt.clear();
            }
            Err(err) => {
                self.history.pop();
                self.reply = format!("erro de inferência: {err}");
            }
        }
    }

    /// Nova sessão: limpa histórico, prompt e resposta.
    pub fn clear_session(&mut self) {
        self.history.clear();
        self.prompt.clear();
        self.reply.clear();
    }
}

impl Default for InferenceState {
    fn default() -> Self {
        Self::new()
    }
}

/// Lista os modelos do servidor (`GET /v1/models`).
pub fn list_models(base_url: &str) -> Result<Vec<ModelInfo>, InferenceError> {
    let url = base_url.trim_end_matches('/');
    let response = send(url, client(url)?.get(format!("{url}/v1/models")))?;
    #[derive(Deserialize)]
    struct List {
        data: Vec<ModelInfo>,
    }
    response
        .json::<List>()
        .map(|list| list.data)
        .map_err(|err| InferenceError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })
}

/// Geração real (`POST /v1/chat/completions`); retorna o conteúdo da 1ª escolha.
pub fn chat_completion(base_url: &str, chat: &ChatRequest) -> Result<String, InferenceError> {
    let url = base_url.trim_end_matches('/');
    let response = send(
        url,
        client(url)?
            .post(format!("{url}/v1/chat/completions"))
            .json(chat),
    )?;
    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(Deserialize)]
    struct Completion {
        choices: Vec<Choice>,
    }
    let completion: Completion = response.json().map_err(|err| InferenceError::Unreachable {
        url: String::from(url),
        detail: err.to_string(),
    })?;
    completion
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or(InferenceError::EmptyChoices)
}
