//! Smoke test T-302: ciclo de vida do `DataSpace` real.
//! Sobe, escreve/lê uma task no próprio dataspace, derruba sem vazar.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::DataSpace;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 81;

fn make_task(id: &str) -> Task {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    Task {
        task_id: id.into(),
        client_id: "smoke".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 32,
        stream: false,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dataspace_lifecycle_smoke() {
    // Sobe
    let ds = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("DataSpace sobe");
    assert_eq!(ds.ownership_strength(), 200);

    // Settle mínimo para o match local
    tokio::time::sleep(Duration::from_millis(1200)).await;

    // Escreve + lê de volta (self-loop no mesmo participant)
    ds.write_task_sync(&make_task("smoke-task-1"))
        .expect("write");
    let mut got = Vec::new();
    for _ in 0..50 {
        got = ds.take_tasks_sync().expect("take");
        if !got.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(got.len(), 1, "esperava 1 task, veio {}", got.len());
    assert_eq!(got[0].task_id, "smoke-task-1");
    assert_eq!(got[0].client_id, "smoke");

    // Derruba sem vazar (drop ordenado; segunda chamada de shutdown não existe —
    // consome o valor, provando que o ciclo de vida é RAII)
    ds.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dataspace_two_instances_different_domains() {
    // Dois DataSpaces em domínios distintos sobem lado a lado (isolamento).
    let a = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_AGENT).expect("ds A");
    let b = DataSpace::new(DOMAIN + 2, DataSpace::STRENGTH_CLIENT).expect("ds B");
    assert_eq!(a.ownership_strength(), 100);
    assert_eq!(b.ownership_strength(), 10);
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}
