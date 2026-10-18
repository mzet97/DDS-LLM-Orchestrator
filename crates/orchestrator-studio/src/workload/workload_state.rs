//! Parte de `workload` (extraída para manter os arquivos revisáveis).

use super::*;

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
