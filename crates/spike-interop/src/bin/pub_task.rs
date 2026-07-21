//! Publica N Tasks via DDS e sai (REQ-101).
//!
//! Uso: cargo run --bin pub-task -- [--count N] [--domain ID]

use anyhow::Result;
use cyclonedds::{DataWriter, DdsEntity, DomainParticipant, Publisher, Topic};
use dds_contract::generated::dds_llm_orchestrator::Task;
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
        .unwrap_or(10);

    let domain_id: u32 = std::env::args()
        .find(|a| a == "--domain")
        .and_then(|_| {
            std::env::args()
                .skip_while(|a| a != "--domain")
                .nth(1)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0);

    println!("[pub-task] Iniciando no domínio {domain_id}, publicando {count} Tasks");

    let dp = DomainParticipant::new(domain_id)?;
    let qos = profiles::tasks(Some(200))?; // strength 200 > 100 do writer ocioso do stub Python

    let topic = Topic::<Task>::with_qos(dp.entity(), topics::TASKS, Some(&qos))?;
    let publisher = Publisher::new(dp.entity())?;
    let writer = DataWriter::with_qos(publisher.entity(), topic.entity(), Some(&qos))?;

    // Aguarda discovery/SEDp casar com os readers (QoS Volatile: amostras
    // escritas antes do match são descartadas).
    std::thread::sleep(std::time::Duration::from_millis(2500));

    let now_ns = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64;

    for i in 0..count {
        let task = Task {
            task_id: format!("spike-task-{i:04}"),
            client_id: "spike-client".into(),
            assigned_agent: "".into(),
            target_agent: "".into(),
            model_required: 0,
            model_name: "qwen3.5-0.8b".into(),
            messages_json: r#"[{"role":"user","content":"Hello from Rust!"}]"#.into(),
            temperature: 0.7,
            max_tokens: 256,
            stream: false,
            status: 0,   // PENDING
            priority: 1, // NORMAL
            created_at_ns: now_ns + i as u64,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns: now_ns + 60_000_000_000, // +60s
            retry_count: 0,
            finish_reason: "".into(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        };

        writer.write(&task)?;
        println!(
            "[pub-task] Task {} publicada: task_id={}",
            i + 1,
            task.task_id
        );
    }

    println!("[pub-task] Concluído: {count} Tasks publicadas");

    // Aguarda ACKs dos readers confiáveis antes de destruir o writer.
    // 10s cobre o caso em que o match com o peer demora (TypeLookup/resolução
    // de tipo): com TransientLocal, as amostras são entregues quando o match forma.
    let _ = writer.wait_for_acks(10_000_000_000); // 10s em ns
    Ok(())
}
