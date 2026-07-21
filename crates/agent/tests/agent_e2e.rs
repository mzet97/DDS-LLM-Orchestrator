//! Teste E2E do agente (T-202/203/204/206): claim → confirma → RUNNING → DONE
//! com chunks publicados pelo pool e heartbeat no AgentRegistry.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p agent --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use agent::claim::Specialization;
use agent::dds::AgentDds;
use agent::engine::MockEngine;
use agent::AgentConfig;
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 90;

fn make_task(id: &str) -> Task {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    Task {
        task_id: id.into(),
        client_id: "e2e-client".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen".into(),
        messages_json: r#"[{"role":"user","content":"oi"}]"#.into(),
        temperature: 0.7,
        max_tokens: 16,
        stream: true,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn agent_claim_process_complete_e2e() {
    // "Cliente" que publica tasks (strength 10 — em produção, o orquestrador
    // NÃO publica tasks; se publicasse com strength 200, os claims dos agentes
    // (100) perderiam a arbitragem de Exclusive ownership).
    let orq = DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).unwrap();

    // Alimenta o cache de outputs do orquestrador via stream dedicada
    let mut out_stream = Box::pin(orq.stream_task_outputs());
    let out_feeder = tokio::spawn(async move { while out_stream.next().await.is_some() {} });

    // Alimenta o cache do AgentRegistry (orquestrador assina o registry)
    let mut agent_stream = Box::pin(orq.stream_agent_states());
    let agent_feeder = tokio::spawn(async move { while agent_stream.next().await.is_some() {} });

    // Agente Rust com MockEngine (3 chunks, 10 ms/chunk)
    let config = AgentConfig {
        agent_id: "agent-e2e-1".into(),
        hostname: "testhost".into(),
        model: "qwen".into(),
        specialization: Specialization::Text,
        slots: 4,
        dds_domain: DOMAIN,
    };
    let runtime = Arc::new(AgentDds::new(config).unwrap());
    let _hb = runtime.clone().spawn_heartbeat();
    let engine = Arc::new(MockEngine::new("chunk", 3, 10));
    let runtime2 = Arc::clone(&runtime);
    let claim_loop = tokio::spawn(async move { runtime2.run(engine).await });

    // Settle: agente assina Tasks
    tokio::time::sleep(Duration::from_millis(2500)).await;

    // Submete 10 tasks
    for i in 0..10 {
        orq.write_task(make_task(&format!("e2e-task-{i}")))
            .await
            .unwrap();
    }

    // Espera as 10 chegarem a DONE (status 3) com assigned_agent=agent-e2e-1
    let mut completed = HashSet::new();
    let mut stream = Box::pin(orq.stream_tasks());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(40);
    while completed.len() < 10 && tokio::time::Instant::now() < deadline {
        if let Ok(Some(t)) = tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            if t.status == 3 && t.task_id.starts_with("e2e-task-") {
                assert_eq!(
                    t.assigned_agent, "agent-e2e-1",
                    "task completada por outro agente"
                );
                completed.insert(t.task_id.clone());
            }
        }
    }
    assert_eq!(completed.len(), 10, "tasks que completaram: {completed:?}");

    // Outputs: 3 chunks × 10 tasks publicados
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut n_out = 0;
    for i in 0..10 {
        n_out += orq
            .read_task_outputs(&format!("e2e-task-{i}"))
            .await
            .unwrap()
            .len();
    }
    assert_eq!(n_out, 30, "esperava 30 outputs (3×10), veio {n_out}");

    // Heartbeat: agente presente no AgentRegistry com 10 completions
    tokio::time::sleep(Duration::from_millis(5500)).await;
    let agents = orq.all_agents().await.unwrap();
    let st = agents
        .iter()
        .find(|a| a.agent_id == "agent-e2e-1")
        .expect("heartbeat não publicou AgentState");
    assert_eq!(st.slots_total, 4);
    assert_eq!(st.completed_total, 10, "completed_total no AgentState");
    assert!(st.uptime_seconds > 0, "uptime_seconds deveria ser > 0");

    claim_loop.abort();
    out_feeder.abort();
    agent_feeder.abort();
    drop(stream);
    orq.shutdown().await.unwrap();
}
