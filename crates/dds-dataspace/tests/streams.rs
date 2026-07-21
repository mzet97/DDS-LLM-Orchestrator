//! Teste T-304: streams por evento (WaitSet via `take_aiter`), sem busy-wait.
//! Mede a latência de wakeup (write → stream recebe) e alimenta os caches.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 82;

fn make_task(id: &str) -> Task {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    Task {
        task_id: id.into(),
        client_id: "stream".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 32,
        stream: false,
        status: 0,
        priority: 5,
        created_at_ns: now_ns,
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: now_ns + 60_000_000_000,
        retry_count: 0,
        finish_reason: String::new(),
        t_serialization_ns: 0,
        t_transport_send_ns: 0,
        t_agent_queue_ns: 0,
        t_inference_ns: 0,
        t_transport_return_ns: 0,
        t_deserialization_ns: 0,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subscribe_tasks_wakeup_por_amostra() {
    let ds_sub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_pub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();

    let mut stream = Box::pin(ds_sub.stream_tasks());

    // Settle: SEDP/match entre os dois dataspaces
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Mede latência de wakeup para 20 amostras
    const N: usize = 20;
    let mut latencias_ms = Vec::with_capacity(N);
    for i in 0..N {
        let t0 = Instant::now();
        ds_pub
            .write_task_sync(&make_task(&format!("wake-{i}")))
            .unwrap();
        let got = tokio::time::timeout(Duration::from_secs(3), stream.next())
            .await
            .expect("stream deveria acordar por amostra")
            .expect("stream aberto");
        latencias_ms.push(t0.elapsed().as_secs_f64() * 1e3);
        assert_eq!(got.task_id, format!("wake-{i}"));
        assert!(got.status == 0);
    }

    latencias_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = latencias_ms[N / 2];
    let p99 = latencias_ms[(N as f64 * 0.99) as usize];
    println!("[T-304] wakeup latency: p50={p50:.3} ms, p99={p99:.3} ms (n={N})");

    // Cache alimentado pela stream
    assert_eq!(ds_sub.caches().all_tasks().len(), N);

    // Orçamento duro de sanidade (o medido real fica ~0.3-5 ms)
    assert!(p50 < 50.0, "p50 de wakeup acima do aceitável: {p50} ms");

    drop(stream); // libera o borrow do subscribe antes do shutdown (consome o DataSpace)
    ds_sub.shutdown().await.unwrap();
    ds_pub.shutdown().await.unwrap();
}

/// T-308: bench de propagação de estado (write → visível no assinante), 500 amostras.
/// Orçamento do ROADMAP: < 5 ms p99. Baseline Python (spike benchmark): p50 ~19 ms.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bench_propagacao_de_estado_500() {
    let ds_sub = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_pub = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();

    let mut stream = Box::pin(ds_sub.stream_tasks());
    tokio::time::sleep(Duration::from_millis(2000)).await;

    const N: usize = 500;
    let mut latencias_us = Vec::with_capacity(N);
    for i in 0..N {
        let t0 = Instant::now();
        ds_pub
            .write_task_sync(&make_task(&format!("prop-{i}")))
            .unwrap();
        let got = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("stream deveria acordar")
            .expect("stream aberto");
        latencias_us.push(t0.elapsed().as_secs_f64() * 1e6);
        assert_eq!(got.task_id, format!("prop-{i}"));
    }

    latencias_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let to_ms = |v: f64| v / 1000.0;
    let p50 = to_ms(latencias_us[N / 2]);
    let p95 = to_ms(latencias_us[(N as f64 * 0.95) as usize]);
    let p99 = to_ms(latencias_us[(N as f64 * 0.99) as usize]);
    let mean = to_ms(latencias_us.iter().sum::<f64>() / N as f64);
    println!(
        "[T-308] propagação de estado (n={N}): p50={p50:.3} ms, mean={mean:.3} ms, p95={p95:.3} ms, p99={p99:.3} ms (orçamento p99<5 ms)"
    );

    assert!(p99 < 5.0, "p99 de propagação acima do orçamento: {p99} ms");

    drop(stream);
    ds_sub.shutdown().await.unwrap();
    ds_pub.shutdown().await.unwrap();
}
