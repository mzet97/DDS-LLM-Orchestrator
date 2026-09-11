//! Atendimento de uma `LLM.InferenceRequest`: valida, deriva a fixture,
//! publica chunks/erro e registra o resultado no log JSONL.

use crate::fixture::{self, Invalid};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::Sender;

pub fn wall_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub type Log = std::sync::Mutex<std::fs::File>;

pub fn log_record(log: &Log, rec: serde_json::Value) {
    if let Ok(mut f) = log.lock() {
        let _ = writeln!(f, "{rec}");
        let _ = f.flush();
    }
}

/// Resultado registrado por requisição atendida, rejeitada ou falha.
pub struct Report<'a> {
    pub seq: u64,
    pub req: &'a LLMInferenceRequest,
    pub stage: &'a str,
    pub family: &'a str,
    pub hash: &'a str,
    pub n: usize,
    pub arrival: Duration,
    pub started: Duration,
    pub delay_ms: u64,
    pub status: &'a str,
    pub detail: Option<String>,
}

fn outcome(log: &Log, finished: Duration, r: Report<'_>) {
    let mut rec = serde_json::json!({
        "seq": r.seq,
        "request_id": r.req.request_id,
        "task_id": r.req.task_id,
        "agent_id": r.req.agent_id,
        "model": r.req.model_name,
        "stage": r.stage,
        "family": r.family,
        "hash": r.hash,
        "n_msgs": r.n,
        "arrival_ns": r.arrival.as_nanos() as u64,
        "started_ns": r.started.as_nanos() as u64,
        "finished_ns": finished.as_nanos() as u64,
        "delay_ms_config": r.delay_ms,
        "outcome": r.status,
    });
    if let Some(d) = r.detail {
        rec["error"] = serde_json::Value::String(d);
    }
    log_record(log, rec);
}

pub struct Endpoint {
    pub res_writer: Arc<cyclonedds::DataWriter<LLMInferenceResult>>,
    pub err_writer: Arc<cyclonedds::DataWriter<LLMInferenceError>>,
    pub t0: Instant,
    pub delay_ms: u64,
    pub model_only: Option<String>,
    pub log: Arc<Log>,
    pub seq: AtomicU64,
    pub n_ok: AtomicU64,
    pub n_err: AtomicU64,
    pub n_rejected: AtomicU64,
}

impl Endpoint {
    pub fn publish_error(&self, req: &LLMInferenceRequest, code: i32, msg: &str, retriable: bool) {
        let _ = self.err_writer.write(&LLMInferenceError {
            request_id: req.request_id.clone(),
            error_code: code,
            error_message: msg.to_string(),
            provider: "det-responder".to_string(),
            retriable,
            emitted_at_ns: wall_ns(),
        });
    }

    fn finish(&self, r: Report<'_>) {
        outcome(&self.log, self.t0.elapsed(), r);
    }

    fn blank<'a>(
        &self,
        req: &'a LLMInferenceRequest,
        seq: u64,
        arrival: Duration,
        started: Duration,
    ) -> Report<'a> {
        Report {
            seq,
            req,
            stage: "",
            family: "",
            hash: "",
            n: 0,
            arrival,
            started,
            delay_ms: self.delay_ms,
            status: "error",
            detail: None,
        }
    }

    fn fail(
        &self,
        req: &LLMInferenceRequest,
        seq: u64,
        arrival: Duration,
        started: Duration,
        invalid: Invalid,
    ) {
        self.publish_error(req, 400, invalid.message(), false);
        self.n_err.fetch_add(1, Ordering::SeqCst);
        let mut r = self.blank(req, seq, arrival, started);
        r.detail = Some(invalid.message().to_string());
        self.finish(r);
    }

    /// Enfileira a requisição ou publica erro 429 observável se a fila lotou.
    pub fn admit(&self, tx: &Sender<(LLMInferenceRequest, Duration)>, req: LLMInferenceRequest) {
        let arrival = self.t0.elapsed();
        if tx.try_send((req.clone(), arrival)).is_err() {
            self.publish_error(&req, 429, "responder saturado", true);
            self.n_rejected.fetch_add(1, Ordering::SeqCst);
            let mut r = self.blank(
                &req,
                self.seq.fetch_add(1, Ordering::SeqCst) + 1,
                arrival,
                arrival,
            );
            r.status = "rejected_full";
            r.detail = Some("fila cheia".to_string());
            self.finish(r);
        }
    }

    pub async fn handle(&self, req: LLMInferenceRequest, arrival: Duration) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let started = self.t0.elapsed();
        let parsed = match fixture::parse(&req.messages_json) {
            Ok(p) => p,
            Err(invalid) => {
                self.fail(&req, seq, arrival, started, invalid);
                return;
            }
        };
        if let Some(only) = &self.model_only {
            if req.model_name != *only {
                let msg = format!("model '{}' fora de --model '{only}'", req.model_name);
                self.publish_error(&req, 400, &msg, false);
                self.n_err.fetch_add(1, Ordering::SeqCst);
                let mut r = self.blank(&req, seq, arrival, started);
                r.detail = Some(msg);
                self.finish(r);
                return;
            }
        }
        if req.model_name.is_empty() {
            self.fail(&req, seq, arrival, started, Invalid::EmptyModel);
            return;
        }
        // Atraso configurado aplicado após o início (espelha o backend HTTP).
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        let h = fixture::content_hash(&parsed.normalized);
        let content = fixture::fixture_text(parsed.stage, &h, parsed.messages.len());
        let mid = fixture::split_midpoint(&content);
        let emitted = wall_ns();
        let chunks = [
            LLMInferenceResult {
                request_id: req.request_id.clone(),
                seq_num: 0,
                content: content[..mid].to_string(),
                is_final: false,
                finish_reason: 0,
                model_used: req.model_name.clone(),
                tokens_prompt: 0,
                tokens_completion: 0,
                emitted_at_ns: emitted,
            },
            LLMInferenceResult {
                request_id: req.request_id.clone(),
                seq_num: 1,
                content: content[mid..].to_string(),
                is_final: true,
                finish_reason: 1,
                model_used: req.model_name.clone(),
                tokens_prompt: 10,
                tokens_completion: 5,
                emitted_at_ns: emitted,
            },
        ];
        for c in &chunks {
            if let Err(e) = self.res_writer.write(c) {
                let msg = format!("falha ao publicar resultado: {e}");
                self.publish_error(&req, 500, &msg, true);
                self.n_err.fetch_add(1, Ordering::SeqCst);
                self.finish(Report {
                    seq,
                    req: &req,
                    stage: parsed.stage,
                    family: parsed.family,
                    hash: &h,
                    n: parsed.messages.len(),
                    arrival,
                    started,
                    delay_ms: self.delay_ms,
                    status: "error",
                    detail: Some(msg),
                });
                return;
            }
        }
        self.n_ok.fetch_add(1, Ordering::SeqCst);
        self.finish(Report {
            seq,
            req: &req,
            stage: parsed.stage,
            family: parsed.family,
            hash: &h,
            n: parsed.messages.len(),
            arrival,
            started,
            delay_ms: self.delay_ms,
            status: "ok",
            detail: None,
        });
    }
}
