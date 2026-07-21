//! Publica N TaskOutput com seq_num crescente (REQ-105).
//!
//! Uso: cargo run --bin pub-stream -- [--count N] [--domain ID]

use anyhow::Result;
use cyclonedds::{DataWriter, DdsEntity, DomainParticipant, Publisher, Topic};
use dds_contract::generated::dds_llm_orchestrator::TaskOutput;
use dds_contract::topics;
use spike_interop::profiles;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let count: usize = std::env::args()
        .find(|a| a == "--count")
        .and_then(|_| {
            std::env::args()
                .skip_while(|a| a != "--count")
                .nth(1)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(1000);

    let domain_id: u32 = std::env::args()
        .find(|a| a == "--domain")
        .and_then(|_| {
            std::env::args()
                .skip_while(|a| a != "--domain")
                .nth(1)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0);

    println!("[pub-stream] Iniciando no domínio {domain_id}, publicando {count} chunks");

    let dp = DomainParticipant::new(domain_id)?;
    let qos = profiles::task_output(Some(200))?; // strength 200 > 100 do writer ocioso do stub Python

    let topic = Topic::<TaskOutput>::with_qos(dp.entity(), topics::TASK_OUTPUT, Some(&qos))?;
    let publisher = Publisher::new(dp.entity())?;
    let writer = DataWriter::with_qos(publisher.entity(), topic.entity(), Some(&qos))?;

    // O stub Python lê outputs via `all_tasks()` (tópico Tasks): publica a Task
    // dona do stream (como o agente de produção faz antes de streamar).
    let qos_tasks = spike_interop::profiles::tasks(Some(200))?;
    let tasks_topic = Topic::<dds_contract::generated::dds_llm_orchestrator::Task>::with_qos(
        dp.entity(),
        topics::TASKS,
        Some(&qos_tasks),
    )?;
    let tasks_writer =
        DataWriter::with_qos(publisher.entity(), tasks_topic.entity(), Some(&qos_tasks))?;

    // Aguarda discovery/SEDp casar com os readers (Volatile: chunks
    // escritos antes do match seriam descartados → gaps falsos).
    std::thread::sleep(std::time::Duration::from_millis(2500));

    let now_ns = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64;

    let task_id = format!("spike-stream-{now_ns}");

    tasks_writer.write(&dds_contract::generated::dds_llm_orchestrator::Task {
        task_id: task_id.clone(),
        client_id: "spike-client".into(),
        assigned_agent: "spike-agent-rust".into(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: r#"[{"role":"user","content":"stream"}]"#.into(),
        temperature: 0.7,
        max_tokens: 256,
        stream: true,
        status: 2, // RUNNING
        priority: 5,
        created_at_ns: now_ns,
        assigned_at_ns: now_ns,
        started_at_ns: now_ns,
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
    })?;
    println!("[pub-stream] Task do stream publicada: task_id={task_id}");

    for i in 0..count {
        let is_final = i == count - 1;
        let output = TaskOutput {
            task_id: task_id.clone(),
            seq_num: i as u32,
            content: format!("chunk-{i:04}"),
            is_final,
            finish_reason: if is_final { 1 } else { 0 }, // COMPLETION
            agent_id: "spike-agent-rust".into(),
            token_count: 1,
            emitted_at_ns: now_ns + i as u64,
        };

        writer.write(&output)?;

        if i % 100 == 0 {
            println!("[pub-stream] Chunk {i}/{count} publicado: seq_num={i}");
        }
    }

    println!("[pub-stream] Concluído: {count} chunks publicados, task_id={task_id}");

    // Aguarda ACKs dos readers antes de destruir o writer (entrega confiável).
    let _ = writer.wait_for_acks(10_000_000_000); // 10s em ns
    Ok(())
}
