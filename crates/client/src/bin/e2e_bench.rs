//! Driver mínimo de comparação E2E real (Rust vs Python) — sem mocks.
//!
//! Submete N tasks reais via DDS (`DdsClientDds`) contra um `llama-server`
//! real + agente(s) real(is) já rodando, e reporta p50/p95/p99 de latência.
//! Ver `tese/src/rust/OPTIMIZATION_REPORT.md` §"Comparação real E2E".
//!
//! Uso (sequencial): `... --bin e2e-bench -- <domain> <n>`
//! Uso (concorrente, Fase R6): `... --bin e2e-bench -- <domain> <n> --concurrent`
//! — as N tasks são submetidas TODAS AO MESMO TEMPO (`tokio::spawn`), no MESMO
//! `DdsClientDds` compartilhado (mesmo padrão de `client::submit()` em produção
//! — cada submissão já cria seus próprios streams `stream_tasks`/
//! `stream_task_outputs`, ver Fase 5/`dispatch::SharedWaitSet`).

use client::dds_impl::DdsClientDds;
use client::{ClientConfig, DdsClient};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let domain: u32 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(77);
    let n: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(20);
    let concurrent = args.iter().any(|a| a == "--concurrent");

    let config = ClientConfig {
        client_id: "e2e-bench-rust".to_string(),
        dds_domain: domain,
        timeout_ms: 60_000,
    };
    let helper = DdsClient::new(config.clone());
    let client = Arc::new(DdsClientDds::new(config)?);

    let messages_json = serde_json::json!([
        {"role": "user", "content": "What is 2+2? Answer with just the number."}
    ])
    .to_string();

    let mut latencies_ms: Vec<u64> = Vec::with_capacity(n);
    let mut failures = 0usize;

    if concurrent {
        let mut handles = Vec::with_capacity(n);
        for i in 0..n {
            let task = helper.create_task("qwen3.5-0.8b", &messages_json, 5, false);
            let client = Arc::clone(&client);
            handles.push(tokio::spawn(async move {
                let r = client.submit(task).await;
                (i, r)
            }));
        }
        for h in handles {
            let (i, r) = h.await?;
            match r {
                Ok(result) => {
                    eprintln!(
                        "[{i}] ok latency_ms={} tokens_completion={} content={:?}",
                        result.latency_ms,
                        result.tokens_completion,
                        result.content.chars().take(40).collect::<String>()
                    );
                    latencies_ms.push(result.latency_ms);
                }
                Err(e) => {
                    eprintln!("[{i}] FAIL: {e}");
                    failures += 1;
                }
            }
        }
    } else {
        for i in 0..n {
            let task = helper.create_task("qwen3.5-0.8b", &messages_json, 5, false);
            match client.submit(task).await {
                Ok(result) => {
                    eprintln!(
                        "[{i}] ok latency_ms={} tokens_completion={} content={:?}",
                        result.latency_ms,
                        result.tokens_completion,
                        result.content.chars().take(40).collect::<String>()
                    );
                    latencies_ms.push(result.latency_ms);
                }
                Err(e) => {
                    eprintln!("[{i}] FAIL: {e}");
                    failures += 1;
                }
            }
        }
    }

    latencies_ms.sort_unstable();
    let pct = |p: f64| -> u64 {
        if latencies_ms.is_empty() {
            return 0;
        }
        let idx = ((p / 100.0) * (latencies_ms.len() as f64 - 1.0)).round() as usize;
        latencies_ms[idx.min(latencies_ms.len() - 1)]
    };
    let mean = if latencies_ms.is_empty() {
        0.0
    } else {
        latencies_ms.iter().sum::<u64>() as f64 / latencies_ms.len() as f64
    };

    println!("RESULT_JSON {{\"n\":{n},\"failures\":{failures},\"mean_ms\":{mean:.1},\"p50_ms\":{},\"p95_ms\":{},\"p99_ms\":{},\"min_ms\":{},\"max_ms\":{}}}",
        pct(50.0), pct(95.0), pct(99.0),
        latencies_ms.first().copied().unwrap_or(0),
        latencies_ms.last().copied().unwrap_or(0));

    Ok(())
}
