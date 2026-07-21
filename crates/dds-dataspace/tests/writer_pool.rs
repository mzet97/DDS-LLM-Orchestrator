//! Testes T-305: pool de writers MPMC (throughput real DDS) + backpressure.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::writer_pool::{WriteRequest, WriterPool};
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 84;

fn make_task(id: &str) -> Task {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    Task {
        task_id: id.into(),
        client_id: "pool".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 16,
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

/// Throughput: 5.000 tasks por 8 threads num pool K=4 — todas chegam ao assinante.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn writer_pool_throughput_5k() {
    let ds_pub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_sub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let pool = ds_pub.new_writer_pool(4, 8_192);

    const N: usize = 5000;
    let counter = Arc::new(AtomicUsize::new(0));

    // Assinante conta as recebidas via stream
    let counter2 = Arc::clone(&counter);
    let mut stream = Box::pin(ds_sub.stream_tasks());
    let collector = tokio::spawn(async move {
        while counter2.load(Ordering::Relaxed) < N {
            if (tokio::time::timeout(Duration::from_secs(30), stream.next()).await).is_ok() {
                counter2.fetch_add(1, Ordering::Relaxed);
            }
        }
        counter2.load(Ordering::Relaxed)
    });

    tokio::time::sleep(Duration::from_millis(2000)).await; // settle/match

    let t0 = Instant::now();
    let mut handles = Vec::new();
    let pool = Arc::new(pool);
    for th in 0..8 {
        let p = Arc::clone(&pool);
        handles.push(std::thread::spawn(move || {
            for i in 0..(N / 8) {
                p.submit(WriteRequest::Task(make_task(&format!("pool-{th}-{i}"))))
                    .expect("submit");
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let got = collector.await.unwrap();
    let elapsed = t0.elapsed();
    println!(
        "[T-305] {} tasks publicadas e recebidas em {:?} ({:.0} tasks/s)",
        got,
        elapsed,
        N as f64 / elapsed.as_secs_f64()
    );
    assert_eq!(got, N);
    assert_eq!(pool.failed(), 0, "backpressure não deveria disparar a 5k");

    let pool = Arc::try_unwrap(pool).ok().expect("pool sem refs");
    pool.drain_and_shutdown();
    ds_pub.shutdown().await.unwrap();
    ds_sub.shutdown().await.unwrap();
}

/// Backpressure: fila minúscula + consumidor lento → submit falha rápido.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_pool_backpressure_fail_fast() {
    // write_fn lento (mock — sem DDS): 50 ms por item
    let slow = Arc::new(|_req: WriteRequest| {
        std::thread::sleep(Duration::from_millis(50));
    });
    let pool = WriterPool::new(1, 4, slow);

    let mut failed = 0;
    for i in 0..16 {
        if pool
            .submit(WriteRequest::Task(make_task(&format!("bp-{i}"))))
            .is_err()
        {
            failed += 1;
        }
    }
    println!("[T-305] backpressure: {failed}/16 submits rejeitados com fila cheia");
    assert!(
        failed > 0,
        "backpressure não disparou com fila de 4 e consumidor lento"
    );
    assert_eq!(pool.failed() as usize, failed);
    pool.drain_and_shutdown();
}
