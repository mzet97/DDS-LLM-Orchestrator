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
pub fn dispatch_sync(
    base_url: &str,
    model: &str,
    prompt: &str,
    temperature: f64,
    max_tokens: u32,
    timeout: std::time::Duration,
) -> Result<DispatchOutcome, DispatchError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
        "max_tokens": max_tokens,
    });
    let response = client
        .post(format!("{url}/api/v1/chat/completions/sync"))
        .json(&body)
        .send()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let http_status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| format!("{}: {}", name, value.to_str().unwrap_or("<não-utf8>"),))
        .collect::<Vec<_>>()
        .join("\n");
    let raw = response
        .error_for_status()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .text()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|err| DispatchError::Unreachable {
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
        // Decomposição T1–T6 (T-850-04/T6) — 0 quando o backend não reporta.
        #[serde(default)]
        t_serialization_ns: u64,
        #[serde(default)]
        t_transport_send_ns: u64,
        #[serde(default)]
        t_agent_queue_ns: u64,
        #[serde(default)]
        t_inference_ns: u64,
        #[serde(default)]
        t_transport_return_ns: u64,
        #[serde(default)]
        t_deserialization_ns: u64,
    }
    let outcome: Outcome =
        serde_json::from_value(value).map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let breakdown = LatencyBreakdown::from_ns(
        outcome.t_serialization_ns,
        outcome.t_transport_send_ns,
        outcome.t_agent_queue_ns,
        outcome.t_inference_ns,
        outcome.t_transport_return_ns,
        outcome.t_deserialization_ns,
    );
    let breakdown = breakdown.is_meaningful().then_some(breakdown);
    match outcome.status.as_str() {
        "completed" => Ok(DispatchOutcome::Completed {
            task_id: outcome.task_id,
            assigned_agent: outcome.assigned_agent,
            latency_ms: outcome.latency_ms,
            content: outcome.content,
            breakdown,
            http_status,
            raw,
            headers,
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

impl DispatchState {
    /// Padrão honesto: orquestrador local, modelo do agente vivo e os
    /// defaults do backend (temp 0.7, max 256, timeout 300 s).
    #[must_use]
    pub fn new() -> Self {
        Self {
            url: String::from("http://127.0.0.1:8085"),
            model: String::from("qwen3.5-0.8b"),
            prompt: String::new(),
            temperature: String::from("0.7"),
            max_tokens: String::from("256"),
            timeout_ms: String::from("300000"),
            selected_tab: 0,
            result: String::new(),
            last: None,
            history: Vec::new(),
            busy: false,
            pending_model: String::new(),
            pending_prompt: String::new(),
            receiver: None,
        }
    }

    /// Despacha em THREAD de trabalho e registra o desfecho em texto (pode
    /// levar minutos: agente real infere — REQ/T-820-19). Clique durante
    /// `busy` é ignorado; `poll` aplica o desfecho no painel. Parâmetros
    /// inválidos barram SEM rede (mesmas faixas do backend + timeout
    /// cliente 1 ms–1 h).
    pub fn send(&mut self) {
        if self.busy {
            return;
        }
        let temperature: f64 = match self.temperature.trim().parse::<f64>() {
            Ok(value) if value.is_finite() && (-2.0..=2.0).contains(&value) => value,
            _ => {
                self.result = format!(
                    "temperatura inválida: {:?} (usar número finito em −2..=2)",
                    self.temperature.trim(),
                );
                return;
            }
        };
        let max_tokens: u32 = match self.max_tokens.trim().parse::<u32>() {
            Ok(value) if (1..=8192).contains(&value) => value,
            _ => {
                self.result = format!(
                    "max_tokens inválido: {:?} (usar inteiro em 1..=8192)",
                    self.max_tokens.trim(),
                );
                return;
            }
        };
        let timeout_ms: u64 = match self.timeout_ms.trim().parse::<u64>() {
            Ok(value) if (1..=3_600_000).contains(&value) => value,
            _ => {
                self.result = format!(
                    "timeout inválido: {:?} (usar inteiro em 1..=3600000 ms)",
                    self.timeout_ms.trim(),
                );
                return;
            }
        };
        let url = self.url.clone();
        let model = self.model.clone();
        let prompt = self.prompt.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = dispatch_sync(
                &url,
                &model,
                &prompt,
                temperature,
                max_tokens,
                std::time::Duration::from_millis(timeout_ms),
            )
            .map_err(|err| err.to_string());
            let _ = tx.send(DispatchMsg::Done(outcome));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.pending_model = self.model.clone();
        self.pending_prompt = self.prompt.clone();
        self.result = "despachando… (agente real infere)".into();
    }

    /// Zera o diagnóstico da sessão (botão RESET; worker órfão em voo
    /// termina sozinho sem tocar a UI — o canal foi descartado).
    pub fn reset(&mut self) {
        self.receiver = None;
        self.busy = false;
        self.result.clear();
        self.last = None;
        self.history.clear();
        self.selected_tab = 0;
        self.pending_model.clear();
        self.pending_prompt.clear();
    }

    /// Repete um despacho do histórico (restaura modelo+prompt e envia).
    pub fn resend(&mut self, index: usize) {
        if let Some(record) = self.history.get(index) {
            self.model = record.model.clone();
            self.prompt = record.prompt.clone();
            self.send();
        }
    }

    /// Drena o worker; chamar a cada frame enquanto `busy`.
    pub fn poll(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.receiver {
            while let Ok(DispatchMsg::Done(outcome)) = rx.try_recv() {
                let timestamp_ms = crate::machines::now_unix_ns() / 1_000_000;
                let model = self.pending_model.clone();
                let prompt = self.pending_prompt.clone();
                match outcome {
                    Ok(DispatchOutcome::Completed {
                        task_id,
                        assigned_agent,
                        latency_ms,
                        content,
                        breakdown,
                        http_status,
                        raw,
                        headers,
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
                            queue_ms: breakdown.map(|b| b.queue_ms),
                            inference_ms: breakdown.map(|b| b.inference_ms),
                            timestamp_ms,
                            model,
                            prompt,
                        });
                        self.last = Some(DispatchOutcome::Completed {
                            task_id,
                            assigned_agent,
                            latency_ms,
                            content,
                            breakdown,
                            http_status,
                            raw,
                            headers,
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
                            queue_ms: None,
                            inference_ms: None,
                            timestamp_ms,
                            model,
                            prompt,
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
                            queue_ms: None,
                            inference_ms: None,
                            timestamp_ms,
                            model,
                            prompt,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Parâmetro inválido barra SEM rede: nada despacha, motivo honesto.
    #[test]
    fn invalid_params_block_without_network() {
        for (temperature, max_tokens, timeout_ms, expect) in [
            ("quente", "256", "300000", "temperatura inválida"),
            ("0.7", "zero", "300000", "max_tokens inválido"),
            ("0.7", "0", "300000", "max_tokens inválido"),
            ("0.7", "9000", "300000", "max_tokens inválido"),
            ("0.7", "256", "muito", "timeout inválido"),
            ("0.7", "256", "0", "timeout inválido"),
            ("99.0", "256", "300000", "temperatura inválida"),
        ] {
            let mut state = DispatchState::new();
            state.temperature = String::from(temperature);
            state.max_tokens = String::from(max_tokens);
            state.timeout_ms = String::from(timeout_ms);
            state.prompt = String::from("oi");
            state.send();
            assert!(
                !state.busy,
                "nada em voo com parâmetro inválido ({temperature}/{max_tokens}/{timeout_ms})"
            );
            assert!(
                state.result.contains(expect),
                "motivo honesto, recebido: {}",
                state.result
            );
        }
    }

    /// RESET zera sessão; Re-enviar restaura modelo+prompt do registro.
    #[test]
    fn reset_clears_and_resend_restores() {
        let mut state = DispatchState::new();
        state.history.push(DispatchRecord {
            task_id: String::from("t-9"),
            agent: None,
            latency_ms: None,
            ok: false,
            detail: String::from("x"),
            queue_ms: None,
            inference_ms: None,
            timestamp_ms: 1,
            model: String::from("mod-r"),
            prompt: String::from("prompt-r"),
        });
        // Porta 0: connect impossível — o worker falha sem pacotes; o
        // RESET órfão antes de qualquer poll (nada polui o histórico).
        state.url = String::from("http://127.0.0.1:0/");
        state.resend(0);
        assert_eq!(state.model, "mod-r");
        assert_eq!(state.prompt, "prompt-r");
        assert!(state.busy, "resend válido entra em voo");
        state.reset();
        assert!(state.history.is_empty());
        assert!(state.last.is_none());
        assert!(state.result.is_empty());
        assert!(!state.busy);
        state.resend(99); // índice inexistente: no-op, sem panic.
    }
}
