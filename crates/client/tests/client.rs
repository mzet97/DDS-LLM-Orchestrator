//! Teste T-411: 50+ submissões concorrentes via UM participante — sem deadlock.
//!
//! O Python travava em 20 (20 participantes × GIL). Aqui: 1 participante,
//! tasks async, agentes Rust processando.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p client --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use agent::claim::Specialization;
use agent::dds::AgentDds;
use agent::engine::MockEngine;
use agent::AgentConfig;
use client::dds_impl::DdsClientDds;
use client::{ClientConfig, DdsClient};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DOMAIN: u32 = 102;

async fn spawn_agent(id: &str) -> Arc<AgentDds> {
    let config = AgentConfig {
        agent_id: id.into(),
        hostname: "testhost".into(),
        model: "qwen".into(),
        specialization: Specialization::Text,
        slots: 8,
        dds_domain: DOMAIN,
    };
    let runtime = Arc::new(AgentDds::new(config).unwrap());
    let engine = Arc::new(MockEngine::new("chunk", 2, 5));
    let r = Arc::clone(&runtime);
    tokio::spawn(async move { r.run(engine).await });
    runtime
}

#[tokio::test(flavor = "multi_thread", worker_threads = 24)]
async fn stress_50_concurrent_submits_um_participante() {
    // 2 agentes Rust com MockEngine
    let _a1 = spawn_agent("stress-agent-1").await;
    let _a2 = spawn_agent("stress-agent-2").await;

    // Cliente: UM participante para as 50 submissões
    let dds_client = Arc::new(
        DdsClientDds::new(ClientConfig {
            client_id: "stress-client".into(),
            dds_domain: DOMAIN,
            timeout_ms: 60_000,
        })
        .unwrap(),
    );

    tokio::time::sleep(Duration::from_millis(2500)).await; // settle

    let base = DdsClient::new(ClientConfig::default());
    const N: usize = 50;
    let t0 = Instant::now();

    let mut set = tokio::task::JoinSet::new();
    for i in 0..N {
        let client = Arc::clone(&dds_client);
        let task = base.create_task("qwen", r#"[{"role":"user","content":"oi"}]"#, 5, true);
        set.spawn(async move { (i, client.submit(task).await) });
    }

    let mut ok = 0;
    let mut failures = Vec::new();
    while let Some(res) = set.join_next().await {
        match res {
            Ok((i, Ok(r))) => {
                assert!(r.success);
                assert!(!r.content.is_empty(), "resultado vazio na task {i}");
                ok += 1;
            }
            Ok((i, Err(e))) => failures.push(format!("task {i}: {e}")),
            Err(e) => failures.push(format!("join: {e}")),
        }
    }

    let elapsed = t0.elapsed();
    println!(
        "[T-411] {ok}/{N} submissões OK em {:?} ({:.1} tasks/s agregado)",
        elapsed,
        N as f64 / elapsed.as_secs_f64()
    );
    assert!(failures.is_empty(), "falhas: {failures:?}");
    assert_eq!(ok, N);
}
