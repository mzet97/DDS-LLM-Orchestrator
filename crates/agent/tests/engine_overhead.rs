//! Medição Fase 3 (9.3): custo do modelo "1 writer + 2 readers + settle por
//! invocação" do `DdsEngine`, sob concorrência 1/4/16. Responder mock DDS
//! responde na hora — o tempo medido é overhead puro do fan-out de entidades.
//!
//! Rode com: `cargo test -p agent --features dds --test engine_overhead -- --test-threads=1 --nocapture`
#![cfg(feature = "dds")]

use agent::engine::{Engine, InferRequest};
use agent::engine_dds::DdsEngine;
use dds_contract::generated::orchestrator::LLMInferenceResult;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Responder mock: para cada LLMInferenceRequest publica 5 chunks + final
/// com o mesmo request_id (sem inferência — overhead do engine isolado).
async fn spawn_responder(ds: DataSpace) {
    tokio::spawn(async move {
        let mut stream = Box::pin(ds.stream_llm_requests());
        while let Some(req) = stream.next().await {
            for seq in 0..5u32 {
                let _ = ds
                    .write_llm_result(LLMInferenceResult {
                        request_id: req.request_id.clone(),
                        seq_num: seq,
                        content: format!("c{seq}"),
                        is_final: seq == 4,
                        finish_reason: if seq == 4 { 1 } else { 0 },
                        model_used: "mock".into(),
                        tokens_prompt: 4,
                        tokens_completion: seq + 1,
                        emitted_at_ns: now_ns(),
                    })
                    .await;
            }
        }
    });
}

fn make_infer(id: &str) -> InferRequest {
    InferRequest {
        request_id: id.into(),
        messages_json: "[]".into(),
        model_name: "mock".into(),
        temperature: 0.7,
        max_tokens: 8,
        stream: true,
        timeout_ms: 15_000,
    }
}

async fn one_call<E: Engine>(engine: &E, id: &str) -> Duration {
    let t0 = Instant::now();
    let mut stream = Box::pin(engine.infer_stream(make_infer(id)));
    let mut chunks = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("chunk ok");
        chunks += 1;
        if chunk.is_final {
            break;
        }
    }
    assert_eq!(chunks, 5, "5 chunks esperados do mock");
    t0.elapsed()
}

/// C concorrências de chamadas simultâneas; retorna tempos por chamada.
async fn measure_concurrency(engine: &DdsEngine, c: usize, tag: &str) -> Vec<Duration> {
    let mut handles = Vec::new();
    for i in 0..c {
        let id = format!("{tag}-{i}");
        handles.push(async move { one_call(engine, &id).await });
    }
    futures_util::future::join_all(handles).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn dds_engine_overhead_por_invocacao() {
    let domain = 104;
    let ds = DataSpace::new(domain, DataSpace::STRENGTH_AGENT).unwrap();
    spawn_responder(ds).await;

    let engine = DdsEngine::new(domain, "agent-bench".into()).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await; // settle/discovery

    // warm-up (primeira request do processo paga discovery extra)
    let warm = one_call(&engine, "warmup").await;

    for c in [1usize, 4, 16] {
        let times = measure_concurrency(&engine, c, &format!("c{c}")).await;
        let mut ms: Vec<f64> = times.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = ms.iter().sum::<f64>() / ms.len() as f64;
        let p95 = ms[((ms.len() as f64) * 0.95).ceil() as usize - 1];
        println!(
            "[9.3] concorrência={c}: warm-up={:.0}ms mean={:.0}ms min={:.0}ms p95={:.0}ms max={:.0}ms",
            warm.as_millis(),
            mean,
            ms[0],
            p95,
            ms[ms.len() - 1]
        );
    }
    // Não asserta thresholds: é MEDIÇÃO (baseline para o dispatcher da Fase 10).
}
