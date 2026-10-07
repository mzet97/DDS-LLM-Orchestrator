//! Despacho no Studio: tarefa real via orquestrador (`POST
//! `/api/v1/chat/completions/sync`), que publica no DDS e aguarda o agente.
//!
//! Resposta crua do backend, sem enfeite: concluída (com agente e latência)
//! ou falha (com motivo). Timeout longo porque agente real infere de verdade.
//! O despacho roda em THREAD de trabalho (padrão `models.rs`: thread com
//! canal mpsc e `poll` por frame) — pode levar até 300 s e a thread de UI
//! nunca bloqueia (REQ/T-820-19).

use serde::Deserialize;
use std::sync::mpsc;
use thiserror::Error;

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
}

/// Erros do despacho (fronteira GUI ↔ orquestrador).
#[derive(Debug, Error)]
pub enum DispatchError {
    /// Orquestrador inalcançável, timeout ou fora do contrato.
    #[error("falha no despacho em {url}: {detail}")]
    Unreachable { url: String, detail: String },
}

/// Despacha o prompt e aguarda a conclusão (`sync`).
pub fn dispatch_sync(
    base_url: &str,
    model: &str,
    prompt: &str,
) -> Result<DispatchOutcome, DispatchError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
    });
    let value: serde_json::Value = client
        .post(format!("{url}/api/v1/chat/completions/sync"))
        .json(&body)
        .send()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .error_for_status()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .json()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    #[derive(Deserialize)]
    struct Outcome {
        task_id: String,
        status: String,
        #[serde(default)]
        assigned_agent: Option<String>,
        #[serde(default)]
        latency_ms: u64,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        content: Option<String>,
    }
    let outcome: Outcome =
        serde_json::from_value(value).map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    match outcome.status.as_str() {
        "completed" => Ok(DispatchOutcome::Completed {
            task_id: outcome.task_id,
            assigned_agent: outcome.assigned_agent,
            latency_ms: outcome.latency_ms,
            content: outcome.content,
        }),
        "failed" => Ok(DispatchOutcome::Failed {
            task_id: outcome.task_id,
            error: outcome
                .error
                .unwrap_or_else(|| String::from("motivo ausente")),
        }),
        other => Err(DispatchError::Unreachable {
            url: String::from(url),
            detail: format!("status desconhecido: {other}"),
        }),
    }
}

/// Mensagem do worker de despacho.
enum DispatchMsg {
    Done(Result<DispatchOutcome, String>),
}

/// Estado do painel de despacho.
#[derive(Debug)]
pub struct DispatchState {
    pub url: String,
    pub model: String,
    pub prompt: String,
    pub result: String,
    /// Último desfecho estruturado (card de resultado, tela 3.6).
    pub last: Option<DispatchOutcome>,
    /// Histórico dos despachos da SESSÃO (mais recente por último).
    pub history: Vec<DispatchRecord>,
    /// `true` enquanto o despacho roda em background (`poll` drena).
    pub busy: bool,
    receiver: Option<mpsc::Receiver<DispatchMsg>>,
}

impl DispatchState {
    /// Padrão honesto: orquestrador local e modelo do agente vivo.
    #[must_use]
    pub fn new() -> Self {
        Self {
            url: String::from("http://127.0.0.1:8085"),
            model: String::from("qwen3.5-0.8b"),
            prompt: String::new(),
            result: String::new(),
            last: None,
            history: Vec::new(),
            busy: false,
            receiver: None,
        }
    }

    /// Despacha em THREAD de trabalho e registra o desfecho em texto (pode
    /// levar minutos: agente real infere — REQ/T-820-19). Clique durante
    /// `busy` é ignorado; `poll` aplica o desfecho no painel.
    pub fn send(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let model = self.model.clone();
        let prompt = self.prompt.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = dispatch_sync(&url, &model, &prompt).map_err(|err| err.to_string());
            let _ = tx.send(DispatchMsg::Done(outcome));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.result = "despachando… (agente real infere)".into();
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(DispatchMsg::Done(outcome)) = rx.try_recv() {
                match outcome {
                    Ok(DispatchOutcome::Completed {
                        task_id,
                        assigned_agent,
                        latency_ms,
                        content,
                    }) => {
                        self.result = format!(
                            "concluída {task_id} agente={} latência={latency_ms}ms",
                            assigned_agent.as_deref().unwrap_or("?")
                        );
                        self.history.push(DispatchRecord {
                            task_id: task_id.clone(),
                            agent: assigned_agent.clone(),
                            latency_ms: Some(latency_ms),
                            ok: true,
                            detail: content.clone().unwrap_or_default(),
                        });
                        self.last = Some(DispatchOutcome::Completed {
                            task_id,
                            assigned_agent,
                            latency_ms,
                            content,
                        });
                    }
                    Ok(DispatchOutcome::Failed { task_id, error }) => {
                        self.result = format!("falha {task_id}: {error}");
                        self.history.push(DispatchRecord {
                            task_id: task_id.clone(),
                            agent: None,
                            latency_ms: None,
                            ok: false,
                            detail: error.clone(),
                        });
                        self.last = Some(DispatchOutcome::Failed { task_id, error });
                    }
                    Err(err) => {
                        self.result = format!("erro de despacho: {err}");
                        self.history.push(DispatchRecord {
                            task_id: String::new(),
                            agent: None,
                            latency_ms: None,
                            ok: false,
                            detail: err.clone(),
                        });
                    }
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
        // Histórico é da sessão: mantém os últimos 20.
        if self.history.len() > 20 {
            let drop = self.history.len() - 20;
            self.history.drain(0..drop);
        }
    }
}

impl Default for DispatchState {
    fn default() -> Self {
        Self::new()
    }
}
