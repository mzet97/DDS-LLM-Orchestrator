//! Benchmark RTT Rust (REQ-104) — metodologia idêntica ao benchmark_rtt.py.
//!
//! Dois participantes no mesmo processo:
//!   - echo: reader em `Tasks`, writer em `TaskOutput` (devolve echo c/ mesmo task_id)
//!   - bench: writer em `Tasks`, reader em `TaskOutput` (mede RTT por amostra)
//!
//! Uso: rtt-bench [--domain ID] [--samples N] [--warmup N]
//! Saída: estatísticas (min/mean/p50/p95/p99/max/stdev) + benchmark_rust_results.json

use anyhow::Result;
use cyclonedds::{
    DataReader, DataWriter, DdsEntity, DomainParticipant, Publisher, Subscriber, Topic,
};
use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput};
use dds_contract::topics;
use spike_interop::profiles;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn make_task(task_id: String, created: u64) -> Task {
    Task {
        task_id,
        client_id: "benchmark".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: r#"[{"role":"user","content":"benchmark"}]"#.into(),
        temperature: 0.7,
        max_tokens: 10,
        stream: false,
        status: 0,   // PENDING
        priority: 5, // NORMAL
        created_at_ns: created,
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: created + 60_000_000_000,
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

fn main() -> Result<()> {
    let domain_id: u32 = arg_u32("--domain", 0);
    let num_samples: usize = arg_u32("--samples", 10_000) as usize;
    let warmup: usize = arg_u32("--warmup", 100) as usize;

    println!("[bench] domínio {domain_id}: {num_samples} amostras + {warmup} warmup");

    // --- lado echo ---
    let stop = Arc::new(AtomicBool::new(false));
    let echo_handle = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || -> Result<()> {
            let dp = DomainParticipant::new(domain_id)?;
            let qos_t = profiles::tasks(None)?;
            let qos_o = profiles::task_output(Some(200))?;
            let t_topic = Topic::<Task>::with_qos(dp.entity(), topics::TASKS, Some(&qos_t))?;
            let o_topic =
                Topic::<TaskOutput>::with_qos(dp.entity(), topics::TASK_OUTPUT, Some(&qos_o))?;
            let sub = Subscriber::new(dp.entity())?;
            let reader =
                DataReader::<Task>::with_qos(sub.entity(), t_topic.entity(), Some(&qos_t))?;
            let pub_ = Publisher::new(dp.entity())?;
            let writer =
                DataWriter::<TaskOutput>::with_qos(pub_.entity(), o_topic.entity(), Some(&qos_o))?;
            let mut echoed: HashSet<String> = HashSet::new();
            while !stop.load(Ordering::Relaxed) {
                if let Ok(samples) = reader.take() {
                    for t in samples {
                        if !t.task_id.starts_with("bench-") && !t.task_id.starts_with("warmup-") {
                            continue;
                        }
                        if !echoed.insert(t.task_id.clone()) {
                            continue;
                        }
                        let out = TaskOutput {
                            task_id: t.task_id,
                            seq_num: 0,
                            content: "echo".into(),
                            is_final: true,
                            finish_reason: 1,
                            agent_id: "rust-echo".into(),
                            token_count: 1,
                            emitted_at_ns: now_ns(),
                        };
                        writer.write(&out)?;
                    }
                }
                std::thread::sleep(Duration::from_micros(200));
            }
            Ok(())
        })
    };

    // --- lado bench ---
    let dp = DomainParticipant::new(domain_id)?;
    let qos_t = profiles::tasks(Some(200))?;
    let qos_o = profiles::task_output(None)?;
    let t_topic = Topic::<Task>::with_qos(dp.entity(), topics::TASKS, Some(&qos_t))?;
    let o_topic = Topic::<TaskOutput>::with_qos(dp.entity(), topics::TASK_OUTPUT, Some(&qos_o))?;
    let pub_ = Publisher::new(dp.entity())?;
    let writer = DataWriter::<Task>::with_qos(pub_.entity(), t_topic.entity(), Some(&qos_t))?;
    let sub = Subscriber::new(dp.entity())?;
    let reader = DataReader::<TaskOutput>::with_qos(sub.entity(), o_topic.entity(), Some(&qos_o))?;

    // Settle: discovery/SEDp + match do par echo.
    std::thread::sleep(Duration::from_millis(3000));

    let mut latencies_ns: Vec<u64> = Vec::with_capacity(num_samples);

    for i in 0..(warmup + num_samples) {
        let warm = i < warmup;
        let task_id = format!(
            "{}-{}-{}",
            if warm { "warmup" } else { "bench" },
            i,
            now_ns()
        );
        let t0 = Instant::now();
        writer.write(&make_task(task_id.clone(), now_ns()))?;

        // Espera o echo (timeout 5s)
        let mut got = false;
        while t0.elapsed() < Duration::from_secs(5) {
            if let Ok(samples) = reader.take() {
                for s in samples {
                    if s.task_id == task_id {
                        got = true;
                        break;
                    }
                }
            }
            if got {
                break;
            }
            std::thread::sleep(Duration::from_micros(100));
        }

        if !got {
            eprintln!("[bench] WARNING: timeout na amostra {i}");
            continue;
        }
        if !warm {
            latencies_ns.push(t0.elapsed().as_nanos() as u64);
        }
        if i % 1000 == 0 && i > 0 {
            println!("[bench] {i}/{} amostras", warmup + num_samples);
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = echo_handle.join();

    // Estatísticas
    latencies_ns.sort_unstable();
    let n = latencies_ns.len();
    if n == 0 {
        eprintln!("[bench] ERRO: nenhuma amostra válida");
        std::process::exit(1);
    }
    let to_ms = |v: u64| v as f64 / 1e6;
    let mean = latencies_ns.iter().sum::<u64>() as f64 / n as f64;
    let stdev = {
        let var = latencies_ns
            .iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>()
            / (n - 1) as f64;
        var.sqrt()
    };
    let p = |q: f64| latencies_ns[(n as f64 * q) as usize];

    println!("\n[bench] === Resultado Rust ===");
    println!("[bench] Amostras válidas: {n}");
    println!("[bench] Mínimo: {:.3} ms", to_ms(latencies_ns[0]));
    println!("[bench] Média: {:.3} ms", to_ms(mean as u64));
    println!("[bench] Mediana (p50): {:.3} ms", to_ms(p(0.50)));
    println!("[bench] p95: {:.3} ms", to_ms(p(0.95)));
    println!("[bench] p99: {:.3} ms", to_ms(p(0.99)));
    println!("[bench] Máximo: {:.3} ms", to_ms(latencies_ns[n - 1]));
    println!("[bench] Desvio padrão: {:.3} ms", to_ms(stdev as u64));

    let json = format!(
        "{{\"samples\": {n}, \"min_ms\": {:.4}, \"mean_ms\": {:.4}, \"median_ms\": {:.4}, \"p95_ms\": {:.4}, \"p99_ms\": {:.4}, \"max_ms\": {:.4}, \"stdev_ms\": {:.4}}}",
        to_ms(latencies_ns[0]),
        to_ms(mean as u64),
        to_ms(p(0.50)),
        to_ms(p(0.95)),
        to_ms(p(0.99)),
        to_ms(latencies_ns[n - 1]),
        to_ms(stdev as u64),
    );
    std::fs::write("benchmark_rust_results.json", &json)?;
    println!("[bench] Resultados salvos em benchmark_rust_results.json");

    Ok(())
}

fn arg_u32(flag: &str, default: u32) -> u32 {
    std::env::args()
        .find(|a| a == flag)
        .and_then(|_| {
            std::env::args()
                .skip_while(|a| a != flag)
                .nth(1)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(default)
}
