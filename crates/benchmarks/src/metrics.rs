//! Registro de request (JSONL) — espelha o núcleo de
//! `benchmarks/qualificacao/src/instrumentation/trace_schema.py::RequestRecord`.
//!
//! Campos não observáveis no caminho DDS Rust (hosts, clock source, network_*)
//! ficam de fora em vez de serem fabricados — mesma regra do runner Python
//! (`status='not_run'` / `measurement_type='inferred'`). A análise
//! (Friedman/mixed models, índice de Jain, etc.) permanece nos scripts Python
//! em `benchmarks/qualificacao/analysis/` — este crate só PRODUZ o JSONL.

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Status terminal da request (subconjunto do Python).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Ok,
    Error,
    Timeout,
    NotRun,
}

/// Registro de uma request (1 linha JSONL por trace_id).
#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    pub trace_id: String,
    pub experiment_id: String,
    pub run_id: String,
    /// Cenário ("e1".."e5", "op1".."op4") — minúsculo, como no Python.
    pub test_id: String,
    /// Transporte — sempre "dds" neste crate.
    pub protocol: String,
    pub agent_id: String,
    pub model_name: String,
    /// Braço de QoS (ex.: "nfcm", "fixed_rules", "ucb1") quando aplicável.
    pub qos_arm: String,
    pub prompt_input_tokens_estimated: u32,
    pub max_output_tokens: u32,
    pub started_at_ns: u64,
    pub finished_at_ns: u64,
    pub status: RequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Regime de carga ("leve"/"moderada"/"pesada") — `load_level` no Python.
    pub load_level: String,
    pub target_rps: f64,
    pub replication_idx: u32,
    pub warmup: bool,
    /// Latência E2E medida no cliente (ms).
    pub latency_ms: f64,
    // ── Componentes de latência (E1) — dos campos t_*_ns do Task (IDL) ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_serialization_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_transport_send_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_agent_queue_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_inference_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_transport_return_ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_deserialization_ns: Option<u64>,
    // ── Streaming (E5) ──
    // Gate A (ADR-001): a unidade observada é o CHUNK de conteúdo visível,
    // não o token — ver docs/tracking/DICIONARIO_METRICAS.md. Aliases
    // mantidos para leitura de raw data antiga.
    #[serde(skip_serializing_if = "Option::is_none", alias = "ttft_ms")]
    pub ttfc_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "itl_mean_ms")]
    pub icl_mean_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_chunks: Option<u32>,
}

/// Escritor JSONL com buffer (append) — paridade com `event_writer.py`.
pub struct JsonlWriter {
    path: PathBuf,
    buf: BufWriter<File>,
    written: u64,
}

impl JsonlWriter {
    /// Abre (cria) o arquivo para append.
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            buf: BufWriter::new(file),
            written: 0,
        })
    }

    /// Grava um registro como linha JSON.
    pub fn append(&mut self, record: &RequestRecord) -> io::Result<()> {
        serde_json::to_writer(&mut self.buf, record).map_err(io::Error::other)?;
        self.buf.write_all(b"\n")?;
        self.written += 1;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.buf.flush()
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for JsonlWriter {
    fn drop(&mut self) {
        let _ = self.buf.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RequestRecord {
        RequestRecord {
            trace_id: "t-1".into(),
            experiment_id: "exp".into(),
            run_id: "run".into(),
            test_id: "e1".into(),
            protocol: "dds".into(),
            agent_id: "agent-1".into(),
            model_name: "phi4".into(),
            qos_arm: "nfcm".into(),
            prompt_input_tokens_estimated: 128,
            max_output_tokens: 50,
            started_at_ns: 1_000,
            finished_at_ns: 2_000,
            status: RequestStatus::Ok,
            error_message: None,
            load_level: "leve".into(),
            target_rps: 5.0,
            replication_idx: 0,
            warmup: false,
            latency_ms: 12.5,
            t_serialization_ns: Some(10),
            t_transport_send_ns: Some(20),
            t_agent_queue_ns: Some(30),
            t_inference_ns: Some(900),
            t_transport_return_ns: Some(20),
            t_deserialization_ns: Some(20),
            ttfc_ms: None,
            icl_mean_ms: None,
            n_chunks: None,
        }
    }

    #[test]
    fn jsonl_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        {
            let mut w = JsonlWriter::create(&path).unwrap();
            w.append(&sample()).unwrap();
            w.append(&sample()).unwrap();
            assert_eq!(w.written(), 2);
            w.flush().unwrap();
        }
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["trace_id"], "t-1");
        assert_eq!(v["status"], "ok");
        assert_eq!(v["protocol"], "dds");
        assert_eq!(v["t_inference_ns"], 900);
        // Campos None são omitidos (não fabricados).
        assert!(v.get("ttfc_ms").is_none());
        assert!(v.get("error_message").is_none());
    }
}
