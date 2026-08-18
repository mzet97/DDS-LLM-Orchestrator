//! Assina Tasks via DDS e imprime/afirma campos (REQ-102).
//!
//! Uso: cargo run --bin sub-task [--domain ID] [--timeout SEC]

use anyhow::Result;
use cyclonedds::{DataReader, DomainParticipant, Subscriber, Topic};
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_contract::topics;
use spike_interop::profiles;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let domain_id: u32 = std::env::args()
        .find(|a| a == "--domain")
        .and_then(|_| {
            std::env::args()
                .skip_while(|a| a != "--domain")
                .nth(1)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0);

    let timeout_secs: u64 = std::env::args()
        .find(|a| a == "--timeout")
        .and_then(|_| {
            std::env::args()
                .skip_while(|a| a != "--timeout")
                .nth(1)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(30);

    println!("[sub-task] Iniciando no domínio {domain_id}, timeout {timeout_secs}s");

    let dp = DomainParticipant::new(domain_id)?;
    let qos = profiles::tasks(None)?;

    let topic = Topic::<Task>::with_qos(&dp, topics::TASKS, Some(&qos))?;
    let subscriber = Subscriber::new(&dp)?;
    let reader = DataReader::<Task>::with_qos(&subscriber, &topic, Some(&qos))?;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let mut count = 0;

    loop {
        if start.elapsed() > timeout {
            println!("[sub-task] Timeout atingido após {timeout_secs}s");
            break;
        }

        match reader.take() {
            Ok(samples) => {
                for sample in samples {
                    count += 1;
                    println!(
                        "[sub-task] Task #{count} recebida: task_id={}, client_id={}, status={}, model={}",
                        sample.task_id, sample.client_id, sample.status, sample.model_name
                    );

                    // Afirma campos obrigatórios
                    assert!(!sample.task_id.is_empty(), "task_id não pode ser vazio");
                    assert!(sample.created_at_ns > 0, "created_at_ns deve ser > 0");

                    println!("[sub-task] ✓ Campos validados com sucesso");
                }
            }
            Err(e) => {
                eprintln!("[sub-task] Erro ao ler: {e}");
            }
        }

        // Pequena pausa para não busy-spin
        std::thread::sleep(Duration::from_millis(10));
    }

    println!("[sub-task] Concluído: {count} Tasks recebidas");
    Ok(())
}
