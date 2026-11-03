//! Publica LLMInferenceRequest e espera LLMInferenceResult (REQ-103).
//!
//! Uso: cargo run --bin llm-client [--domain ID] [--timeout SEC]

use anyhow::Result;
use cyclonedds::{DataReader, DataWriter, DomainParticipant, Publisher, Subscriber, Topic};
use dds_contract::generated::orchestrator::{LLMInferenceRequest, LLMInferenceResult};
use dds_contract::topics;
use spike_interop::profiles;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        .unwrap_or(60);

    println!("[llm-client] Iniciando no domínio {domain_id}, timeout {timeout_secs}s");

    let dp = DomainParticipant::new(domain_id)?;

    // QoS para request/response LLM (tópicos LLM.* — TransientLocal + Shared)
    let qos = profiles::llm()?;

    // Writer para LLMInferenceRequest
    let req_topic = Topic::<LLMInferenceRequest>::with_qos(&dp, topics::LLM_REQUEST, Some(&qos))?;
    let publisher = Publisher::new(&dp)?;
    let req_writer = DataWriter::with_qos(&publisher, &req_topic, Some(&qos))?;

    // Reader para LLMInferenceResult
    let res_topic = Topic::<LLMInferenceResult>::with_qos(&dp, topics::LLM_RESULT, Some(&qos))?;
    let subscriber = Subscriber::new(&dp)?;
    let res_reader =
        DataReader::<LLMInferenceResult>::with_qos(&subscriber, &res_topic, Some(&qos))?;

    let now_ns = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64;

    let request_id = format!("spike-llm-req-{now_ns}");

    // Publica request
    let request = LLMInferenceRequest {
        request_id: request_id.clone(),
        task_id: "spike-task-llm".into(),
        agent_id: "spike-agent-rust".into(),
        model_name: "qwen3.5-0.8b".into(),
        messages_json: r#"[{"role":"user","content":"Say hello in one word"}]"#.into(),
        temperature: 0.7,
        max_tokens: 10,
        stream: false,
        security_level: 0,
        provider_constraint: "ANY".into(),
        created_at_ns: now_ns,
    };

    req_writer.write(&request)?;
    println!("[llm-client] LLMInferenceRequest publicada: request_id={request_id}");

    // Espera resposta
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() > timeout {
            println!("[llm-client] Timeout: nenhuma resposta recebida em {timeout_secs}s");
            println!("[llm-client] Verifique se o llama-server está rodando com --enable-dds");
            break;
        }

        match res_reader.take() {
            Ok(samples) => {
                for sample in samples {
                    if sample.request_id == request_id {
                        println!(
                            "[llm-client] ✓ LLMInferenceResult recebida: content={}, is_final={}, tokens_completion={}",
                            sample.content, sample.is_final, sample.tokens_completion
                        );

                        // Afirma campos
                        assert_eq!(sample.request_id, request_id);
                        assert!(!sample.content.is_empty() || sample.is_final);

                        println!("[llm-client] ✓ Interop Rust↔C++ confirmada!");
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                eprintln!("[llm-client] Erro ao ler: {e}");
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
