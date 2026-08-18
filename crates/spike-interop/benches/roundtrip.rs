//! Benchmark RTT Task → echo TaskOutput (criterion) — REQ-104 / T-105.
//!
//! Mede o round-trip `Tasks` → `TaskOutput` entre dois participantes no mesmo
//! processo, mesma metodologia do `benchmark_rtt.py` e do `bin/rtt_bench.rs`.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo bench -p spike-interop --features dds`

use criterion::{criterion_group, criterion_main, Criterion};
use cyclonedds::{DataReader, DataWriter, DomainParticipant, Publisher, Subscriber, Topic};
use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput};
use dds_contract::topics;
use spike_interop::profiles;
use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Rtt {
    // Entidades mantidas vivas no frame do bench: se o participante
    // (ou tópicos/pub/sub) for dropado, os handles de writer/reader viram inválidos.
    _dp: DomainParticipant,
    _t_topic: Topic<Task>,
    _o_topic: Topic<TaskOutput>,
    _pub: Publisher,
    _sub: Subscriber,
    writer: DataWriter<Task>,
    reader: DataReader<TaskOutput>,
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn setup() -> Rtt {
    const DOMAIN: u32 = 62;

    // Echo (thread separada, detach — vive até o fim do processo de bench)
    std::thread::spawn(move || -> anyhow::Result<()> {
        let dp = DomainParticipant::new(DOMAIN)?;
        let qos_t = profiles::tasks(None)?;
        let qos_o = profiles::task_output(Some(200))?;
        let t_topic = Topic::<Task>::with_qos(&dp, topics::TASKS, Some(&qos_t))?;
        let o_topic = Topic::<TaskOutput>::with_qos(&dp, topics::TASK_OUTPUT, Some(&qos_o))?;
        let sub = Subscriber::new(&dp)?;
        let reader = DataReader::<Task>::with_qos(&sub, &t_topic, Some(&qos_t))?;
        let pub_ = Publisher::new(&dp)?;
        let writer = DataWriter::<TaskOutput>::with_qos(&pub_, &o_topic, Some(&qos_o))?;
        let mut echoed: HashSet<String> = HashSet::new();
        loop {
            if let Ok(samples) = reader.take() {
                for t in samples {
                    if !t.task_id.starts_with("crit-") || !echoed.insert(t.task_id.clone()) {
                        continue;
                    }
                    writer.write(&TaskOutput {
                        task_id: t.task_id,
                        seq_num: 0,
                        content: "echo".into(),
                        is_final: true,
                        finish_reason: 1,
                        agent_id: "rust-echo".into(),
                        token_count: 1,
                        emitted_at_ns: now_ns(),
                    })?;
                }
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    });

    // Bench side
    let dp = DomainParticipant::new(DOMAIN).expect("participant");
    let qos_t = profiles::tasks(Some(200)).expect("qos");
    let qos_o = profiles::task_output(None).expect("qos");
    let t_topic = Topic::<Task>::with_qos(&dp, topics::TASKS, Some(&qos_t)).expect("topic");
    let o_topic =
        Topic::<TaskOutput>::with_qos(&dp, topics::TASK_OUTPUT, Some(&qos_o)).expect("topic");
    let pub_ = Publisher::new(&dp).expect("publisher");
    let writer = DataWriter::<Task>::with_qos(&pub_, &t_topic, Some(&qos_t)).expect("writer");
    let sub = Subscriber::new(&dp).expect("subscriber");
    let reader = DataReader::<TaskOutput>::with_qos(&sub, &o_topic, Some(&qos_o)).expect("reader");

    // Settle: discovery + match do par
    std::thread::sleep(Duration::from_millis(3000));

    Rtt {
        _dp: dp,
        _t_topic: t_topic,
        _o_topic: o_topic,
        _pub: pub_,
        _sub: sub,
        writer,
        reader,
    }
}

fn rtt_once(rtt: &Rtt, i: u64) -> Duration {
    let task_id = format!("crit-{i}-{}", now_ns());
    let task = Task {
        task_id: task_id.clone(),
        client_id: "benchmark".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: r#"[{"role":"user","content":"benchmark"}]"#.into(),
        temperature: 0.7,
        max_tokens: 10,
        stream: false,
        status: 0,
        priority: 5,
        created_at_ns: now_ns(),
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: now_ns() + 60_000_000_000,
        retry_count: 0,
        finish_reason: String::new(),
        t_serialization_ns: 0,
        t_transport_send_ns: 0,
        t_agent_queue_ns: 0,
        t_inference_ns: 0,
        t_transport_return_ns: 0,
        t_deserialization_ns: 0,
    };

    let t0 = Instant::now();
    rtt.writer.write(&task).expect("write");
    while t0.elapsed() < Duration::from_secs(5) {
        if let Ok(samples) = rtt.reader.take() {
            if samples.iter().any(|s| s.task_id == task_id) {
                return t0.elapsed();
            }
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    panic!("timeout esperando echo de {task_id}");
}

fn bench_roundtrip(c: &mut Criterion) {
    let rtt = setup();
    let mut seq: u64 = 0;
    c.bench_function("task_roundtrip_rust", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                seq += 1;
                total += rtt_once(&rtt, seq);
            }
            total
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(200)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(15));
    targets = bench_roundtrip
}
criterion_main!(benches);
