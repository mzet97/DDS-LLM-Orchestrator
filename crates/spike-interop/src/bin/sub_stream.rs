//! Assina TaskOutput e conta gaps em seq_num (REQ-105).
//!
//! Uso: cargo run --bin sub-stream [--domain ID] [--timeout SEC]

use anyhow::Result;
use cyclonedds::{DataReader, DdsEntity, DomainParticipant, Subscriber, Topic};
use dds_contract::generated::dds_llm_orchestrator::TaskOutput;
use dds_contract::topics;
use spike_interop::profiles;
use std::collections::HashSet;
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

    println!("[sub-stream] Iniciando no domínio {domain_id}, timeout {timeout_secs}s");

    let dp = DomainParticipant::new(domain_id)?;
    let qos = profiles::task_output(None)?;

    let topic = Topic::<TaskOutput>::with_qos(dp.entity(), topics::TASK_OUTPUT, Some(&qos))?;
    let subscriber = Subscriber::new(dp.entity())?;
    let reader =
        DataReader::<TaskOutput>::with_qos(subscriber.entity(), topic.entity(), Some(&qos))?;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    let mut received: HashSet<u32> = HashSet::new();
    let mut total_count: usize = 0;
    let mut task_id: Option<String> = None;
    let mut got_final = false;

    loop {
        if start.elapsed() > timeout {
            println!("[sub-stream] Timeout atingido após {timeout_secs}s");
            break;
        }

        match reader.take() {
            Ok(samples) => {
                for sample in samples {
                    total_count += 1;

                    // Registra task_id do primeiro chunk
                    if task_id.is_none() {
                        task_id = Some(sample.task_id.clone());
                        println!(
                            "[sub-stream] Recebendo chunks de task_id={}",
                            sample.task_id
                        );
                    }

                    // Verifica se é da mesma task
                    if Some(&sample.task_id) == task_id.as_ref() {
                        received.insert(sample.seq_num);

                        if sample.is_final {
                            got_final = true;
                            println!(
                                "[sub-stream] Chunk final recebido: seq_num={}",
                                sample.seq_num
                            );
                        }
                    }

                    if total_count % 100 == 0 {
                        println!("[sub-stream] {total_count} chunks recebidos até agora");
                    }
                }
            }
            Err(e) => {
                eprintln!("[sub-stream] Erro ao ler: {e}");
            }
        }

        // Se recebemos o chunk final, podemos calcular os gaps
        if got_final {
            break;
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    // Calcula gaps
    if let Some(tid) = &task_id {
        let max_seq = received.iter().max().copied().unwrap_or(0);
        let expected_count = max_seq + 1;
        let gaps: Vec<u32> = (0..expected_count)
            .filter(|i| !received.contains(i))
            .collect();

        println!("\n[sub-stream] === Resultado ===");
        println!("[sub-stream] Task ID: {tid}");
        println!("[sub-stream] Chunks esperados: {expected_count}");
        println!("[sub-stream] Chunks recebidos: {}", received.len());
        println!("[sub-stream] Total de samples: {total_count}");
        println!("[sub-stream] Gaps encontrados: {}", gaps.len());

        if gaps.is_empty() {
            println!(
                "[sub-stream] ✓ SUCESSO: 0 gaps em {} chunks!",
                received.len()
            );
        } else {
            println!("[sub-stream] ✗ FALHA: {} gaps detectados", gaps.len());
            if gaps.len() <= 20 {
                println!("[sub-stream] Gaps: {gaps:?}");
            } else {
                println!("[sub-stream] Primeiros 20 gaps: {:?}", &gaps[..20]);
            }
            std::process::exit(1);
        }
    } else {
        println!("[sub-stream] Nenhum chunk recebido");
        std::process::exit(1);
    }

    Ok(())
}
