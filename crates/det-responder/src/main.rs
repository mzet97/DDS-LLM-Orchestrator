//! `det-responder` — backend determinístico de benchmark no caminho DDS real.
//!
//! Fiação: tópicos `LLM.*` com os perfis QoS de produção, fila limitada,
//! atendentes limitados e log JSONL. A derivação da fixture vive em
//! [`fixture`]; o atendimento, em [`endpoint`].
//!
//! Uso: `det-responder -- --domain 78 --delay-ms 5 --capacity 4 --log resp.jsonl`

mod endpoint;
mod fixture;

use cyclonedds::{DataReader, DataWriter, DomainParticipant, Publisher, Subscriber, Topic};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
};
use dds_contract::topics;
use dds_dataspace::qos::profiles;
use endpoint::{Endpoint, Log};
use futures_util::StreamExt;
use std::fs::OpenOptions;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Instant;

struct Config {
    domain: u32,
    delay_ms: u64,
    capacity: usize,
    queue: usize,
    model_only: Option<String>,
    log_path: String,
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut cfg = Config {
        domain: 78,
        delay_ms: 5,
        capacity: 4,
        queue: 64,
        model_only: None,
        log_path: String::from("det-responder.jsonl"),
    };
    let mut i = 1;
    while i < args.len() {
        let val = |i: usize| -> Result<String, String> {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} exige valor", args[i]))
        };
        match args[i].as_str() {
            "--domain" => cfg.domain = val(i)?.parse().map_err(|e| format!("domain: {e}"))?,
            "--delay-ms" => cfg.delay_ms = val(i)?.parse().map_err(|e| format!("delay: {e}"))?,
            "--capacity" => cfg.capacity = val(i)?.parse().map_err(|e| format!("capacity: {e}"))?,
            "--queue" => cfg.queue = val(i)?.parse().map_err(|e| format!("queue: {e}"))?,
            "--model" => cfg.model_only = Some(val(i)?),
            "--log" => cfg.log_path = val(i)?,
            other => return Err(format!("argumento desconhecido: {other}")),
        }
        i += 2;
    }
    if cfg.capacity == 0 {
        return Err("capacity deve ser >= 1".to_string());
    }
    Ok(cfg)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = parse_args().map_err(|e| {
        format!("uso: det-responder -- --domain N [--delay-ms D --capacity C --queue Q --model M --log F]\n{e}")
    })?;
    let t0 = Instant::now();

    let participant = DomainParticipant::new(cfg.domain)?;
    let publisher = Publisher::new(&participant)?;
    let subscriber = Subscriber::new(&participant)?;
    let qos = profiles::llm()?;
    let qos_result = profiles::llm_result()?;
    let req_topic =
        Topic::<LLMInferenceRequest>::with_qos(&participant, topics::LLM_REQUEST, Some(&qos))?;
    let res_topic =
        Topic::<LLMInferenceResult>::with_qos(&participant, topics::LLM_RESULT, Some(&qos_result))?;
    let err_topic =
        Topic::<LLMInferenceError>::with_qos(&participant, topics::LLM_ERROR, Some(&qos))?;
    let req_reader =
        DataReader::<LLMInferenceRequest>::with_qos(&subscriber, &req_topic, Some(&qos))?;
    let res_writer = Arc::new(DataWriter::<LLMInferenceResult>::with_qos(
        &publisher,
        &res_topic,
        Some(&qos_result),
    )?);
    let err_writer = Arc::new(DataWriter::<LLMInferenceError>::with_qos(
        &publisher,
        &err_topic,
        Some(&qos),
    )?);

    let log_file: std::fs::File = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.log_path)?;
    let ep = Arc::new(Endpoint {
        res_writer,
        err_writer,
        t0,
        delay_ms: cfg.delay_ms,
        model_only: cfg.model_only,
        log: Arc::new(Log::new(log_file)),
        seq: AtomicU64::new(0),
        n_ok: AtomicU64::new(0),
        n_err: AtomicU64::new(0),
        n_rejected: AtomicU64::new(0),
    });

    let (tx, rx) =
        tokio::sync::mpsc::channel::<(LLMInferenceRequest, std::time::Duration)>(cfg.queue);
    let rx = Arc::new(tokio::sync::Mutex::new(rx));
    for _ in 0..cfg.capacity {
        let rx = Arc::clone(&rx);
        let ep = Arc::clone(&ep);
        tokio::spawn(async move {
            loop {
                let item = { rx.lock().await.recv().await };
                match item {
                    Some((req, arrival)) => ep.handle(req, arrival).await,
                    None => break,
                }
            }
        });
    }

    eprintln!(
        "det-responder READY domain={} delay_ms={} capacity={} queue={} log={}",
        cfg.domain, cfg.delay_ms, cfg.capacity, cfg.queue, cfg.log_path
    );
    let mut req_stream = Box::pin(req_reader.take_aiter_timeout(200_000_000));
    loop {
        tokio::select! {
            batch = req_stream.next() => {
                match batch {
                    Some(Ok(reqs)) => {
                        for req in reqs {
                            ep.admit(&tx, req);
                        }
                    }
                    Some(Err(e)) => eprintln!("det-responder take: {e}"),
                    None => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!(
                    "det-responder STOP ok={} err={} rejected_full={}",
                    ep.n_ok.load(std::sync::atomic::Ordering::SeqCst),
                    ep.n_err.load(std::sync::atomic::Ordering::SeqCst),
                    ep.n_rejected.load(std::sync::atomic::Ordering::SeqCst)
                );
                break;
            }
        }
    }
    Ok(())
}
