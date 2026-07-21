//! Teste de integração DDS (loopback): driver publica tasks, agente real
//! (com `MockEngine`) processa, e o driver grava o JSONL por cenário.
//!
//! Cobre os três padrões de carga: Open (E4), Closed (OP1) e Priority (E3).
//!
//! Rode com:
//! `CYCLONEDDS_STATIC=1 cargo test -p benchmarks --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use agent::claim::Specialization;
use agent::dds::AgentDds;
use agent::engine::MockEngine;
use agent::AgentConfig;
use benchmarks::{get_scenario, BenchmarkDriver, DriverConfig};
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 103;

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

fn cfg(out_dir: std::path::PathBuf, duration_s: f64, seed: u64) -> DriverConfig {
    DriverConfig {
        domain: DOMAIN,
        duration_s,
        seed,
        out_dir,
        model_name: "qwen".into(),
        qos_arm: "nfcm".into(),
        workers: 2,
        timeout_ms: 15_000,
    }
}

fn count_jsonl(path: &std::path::Path) -> (usize, usize) {
    let content = std::fs::read_to_string(path).unwrap();
    let mut total = 0;
    let mut ok = 0;
    for line in content.trim().lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        total += 1;
        if v["status"] == "ok" {
            ok += 1;
        }
    }
    (total, ok)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn driver_open_loop_publica_e_coleta() {
    let _agent = spawn_agent("bench-agent-open").await;
    tokio::time::sleep(Duration::from_millis(2500)).await; // settle

    let dir = tempfile::tempdir().unwrap();
    let scenario = get_scenario("E4").unwrap().clone();
    let driver = BenchmarkDriver::new(cfg(dir.path().to_path_buf(), 4.0, 42), scenario).unwrap();
    let summary = driver.run().await.unwrap();

    assert!(summary.submitted >= 3, "submitted={}", summary.submitted);
    assert!(
        summary.ok >= 3,
        "ok={} errors={} timeouts={}",
        summary.ok,
        summary.errors,
        summary.timeouts
    );
    let (total, ok) = count_jsonl(&summary.out_file);
    assert_eq!(total as u64, summary.ok + summary.errors + summary.timeouts);
    assert_eq!(ok as u64, summary.ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn driver_closed_loop_workers_concorrentes() {
    let _agent = spawn_agent("bench-agent-closed").await;
    tokio::time::sleep(Duration::from_millis(2500)).await;

    let dir = tempfile::tempdir().unwrap();
    let scenario = get_scenario("OP1").unwrap().clone();
    let driver = BenchmarkDriver::new(cfg(dir.path().to_path_buf(), 3.0, 7), scenario).unwrap();
    let summary = driver.run().await.unwrap();

    // 2 workers × 3 s: throughput do MockEngine é alto — esperamos dezenas.
    assert!(summary.submitted >= 10, "submitted={}", summary.submitted);
    assert!(summary.ok >= 10, "ok={}", summary.ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn driver_priority_marca_background_e_injecoes() {
    let _agent = spawn_agent("bench-agent-prio").await;
    tokio::time::sleep(Duration::from_millis(2500)).await;

    let dir = tempfile::tempdir().unwrap();
    let scenario = get_scenario("E3").unwrap().clone();
    let driver = BenchmarkDriver::new(cfg(dir.path().to_path_buf(), 4.0, 11), scenario).unwrap();
    let summary = driver.run().await.unwrap();

    assert!(summary.submitted >= 5, "submitted={}", summary.submitted);
    let content = std::fs::read_to_string(&summary.out_file).unwrap();
    let mut bg = 0;
    let mut high = 0;
    for line in content.trim().lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        match v["load_level"].as_str().unwrap() {
            "background_normal" => bg += 1,
            "injection_high" => high += 1,
            other => panic!("load_level inesperado: {other}"),
        }
    }
    assert!(bg >= 3, "background={bg}");
    assert!(high >= 3, "injeções HIGH={high}");
}
