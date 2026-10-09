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

/// Mensagem do worker de HTTP (modelo de `LaunchMsg`).
enum InferMsg {
    /// Resposta de `Modelos` (`GET /v1/models`) + RTT medido (ms).
    Models(Result<Vec<ModelInfo>, String>, u64),
    /// Resposta de `Enviar` (`POST /v1/chat/completions`) com stats reais.
    Reply(Result<(String, Option<UsageInfo>, u64), String>),
}

/// Estado do painel de inferência: parâmetros que atravessam o contrato,
/// prompt editável e última resposta (ou erro formatado, nunca vazio mudo).
#[derive(Debug)]
pub struct InferenceState {
    pub server_url: String,
    pub model: String,
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    pub prompt: String,
    pub reply: String,
    pub models: Vec<String>,
    pub history: Vec<Message>,
    /// Estatísticas por turno assistente (paralelo ao histórico; tela 3.3).
    pub stats: Vec<TurnStats>,
    /// RTT da última verificação `/v1/models` (ms) — `None` sem verificação
    /// bem-sucedida (a 3.3 mostra "HTTP 200 OK · Nms" só com este valor).
    pub last_verify_ms: Option<u64>,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    /// Momento do envio da geração em curso (timer vivo "GERANDO · N.Ns";
    /// `None` fora de geração) — PRD 3.3.
    pub pending_since: Option<std::time::Instant>,
    receiver: Option<mpsc::Receiver<InferMsg>>,
}

impl InferenceState {
    /// Padrões do mockup 3.3: endpoint do lab, temperatura 0.2, 256 tokens.
    #[must_use]
    pub fn new() -> Self {
        Self {
            server_url: String::from("http://192.168.1.61:8081"),
            model: String::new(),
            temperature: 0.2,
            top_p: 0.95,
            max_tokens: 256,
            prompt: String::new(),
            reply: String::new(),
            models: Vec::new(),
            history: Vec::new(),
            stats: Vec::new(),
            last_verify_ms: None,
            busy: false,
            pending_since: None,
            receiver: None,
        }
    }

    /// Atualiza a lista de modelos em background; erro vira texto no painel,
    /// não panic. Clique durante `busy` é ignorado.
    pub fn refresh_models(&mut self) {
        if self.busy {
            return;
        }
        let url = self.server_url.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let result = list_models(&url).map_err(|err| err.to_string());
            let _ = tx.send(InferMsg::Models(
                result,
                started.elapsed().as_millis() as u64,
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.reply = "listando modelos…".into();
    }

    /// Geração real contra o servidor com o histórico da sessão no fio —
    /// em THREAD de trabalho (até 120 s; REQ/T-820-19). A thread de UI segue
    /// livre: `poll` aplica a resposta ou o erro no painel. Clique durante
    /// `busy` é ignorado.
    pub fn send(&mut self) {
        if self.busy {
            return;
        }
        self.history.push(Message {
            role: Role::User,
            content: self.prompt.clone(),
            ts_unix_ms: crate::machines::now_unix_ns() / 1_000_000,
        });
        let chat = ChatRequest {
            model: self.model.clone(),
            messages: self.history.clone(),
            temperature: self.temperature,
            top_p: self.top_p,
            max_tokens: self.max_tokens,
        };
        let url = self.server_url.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let result = chat_completion_with_stats(&url, &chat)
                .map(|(content, usage)| (content, usage, started.elapsed().as_millis() as u64))
                .map_err(|err| err.to_string());
            let _ = tx.send(InferMsg::Reply(result));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.pending_since = Some(std::time::Instant::now());
        self.reply = "gerando…".into();
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    InferMsg::Models(result, elapsed_ms) => match result {
                        Ok(models) => {
                            if let Some(first) = models.first() {
                                if self.model.is_empty() {
                                    self.model = first.id.clone();
                                }
                            }
                            self.models = models.into_iter().map(|info| info.id).collect();
                            self.last_verify_ms = Some(elapsed_ms);
                            self.reply.clear();
                        }
                        Err(err) => {
                            self.last_verify_ms = None;
                            self.reply = format!("erro ao listar modelos: {err}");
                        }
                    },
                    InferMsg::Reply(result) => match result {
                        Ok((content, usage, elapsed_ms)) => {
                            self.history.push(Message {
                                role: Role::Assistant,
                                content: content.clone(),
                                ts_unix_ms: crate::machines::now_unix_ns() / 1_000_000,
                            });
                            self.stats.push(TurnStats {
                                elapsed_ms,
                                prompt_tokens: usage.as_ref().and_then(|u| u.prompt_tokens),
                                completion_tokens: usage.as_ref().and_then(|u| u.completion_tokens),
                            });
                            self.reply = content;
                            self.prompt.clear();
                        }
                        Err(err) => {
                            self.history.pop();
                            self.reply = format!("erro de inferência: {err}");
                        }
                    },
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
            self.pending_since = None;
        }
    }

    /// Nova sessão: limpa histórico, prompt, resposta e stats.
    pub fn clear_session(&mut self) {
        self.history.clear();
        self.stats.clear();
        self.prompt.clear();
        self.reply.clear();
    }

    /// Interrompe a geração em curso do ponto de vista da UI (botão "Parar
    /// Geração" da 3.3): solta o canal — a thread bloqueante termina sozinha
    /// e a resposta tardia é descartada (o `send` falha sem receptor). Sem
    /// efeito fora de geração.
    pub fn cancel(&mut self) {
        if !self.busy {
            return;
        }
        self.receiver = None;
        self.busy = false;
        self.pending_since = None;
        self.reply = String::from("geração interrompida (resposta tardia descartada)");
    }

    /// Exporta o transcript da sessão (histórico + stats) como JSON em
    /// `dir`, retornando o caminho escrito. Falha de IO propaga ao chamador.
    pub fn export_session(&self, dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        #[derive(serde::Serialize)]
        struct Transcript<'a> {
            history: &'a [Message],
            stats: &'a [TurnStats],
        }
        let payload = serde_json::to_string_pretty(&Transcript {
            history: &self.history,
            stats: &self.stats,
        })
        .map_err(std::io::Error::other)?;
        let name = format!(
            "studio_transcript_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        );
        let path = dir.join(name);
        std::fs::write(&path, payload)?;
        Ok(path)
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

/// Geração real (`POST /v1/chat/completions`); retorna o conteúdo da 1ª
/// escolha (mantida para os callers que só querem o texto).
pub fn chat_completion(base_url: &str, chat: &ChatRequest) -> Result<String, InferenceError> {
    chat_completion_with_stats(base_url, chat).map(|(content, _)| content)
}

/// Como [`chat_completion`], devolvendo também o `usage` reportado pelo
/// servidor (tokens) quando presente na resposta.
pub fn chat_completion_with_stats(
    base_url: &str,
    chat: &ChatRequest,
) -> Result<(String, Option<UsageInfo>), InferenceError> {
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
        #[serde(default)]
        usage: Option<UsageInfo>,
    }
    let completion: Completion = response.json().map_err(|err| InferenceError::Unreachable {
        url: String::from(url),
        detail: err.to_string(),
    })?;
    completion
        .choices
        .into_iter()
        .next()
        .map(|choice| (choice.message.content, completion.usage))
        .ok_or(InferenceError::EmptyChoices)
}
