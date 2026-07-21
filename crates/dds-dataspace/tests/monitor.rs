//! Testes T-306: liveliness nativa + deadline missed, sem polling.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use cyclonedds::{Durability, History, Liveliness, Ownership, Qos, QosBuilder, Reliability};
use dds_contract::generated::dds_llm_orchestrator::{AgentState, TaskOutput};
use dds_dataspace::monitor::{QosEvent, QosMonitor};
use dds_dataspace::DataSpace;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 85;

/// QoS de teste: como o de produção, mas lease de liveliness de 2s (rápido).
fn agents_qos_short_lease() -> Qos {
    QosBuilder::new()
        .reliability(Reliability::Reliable, 10_000_000_000)
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(1))
        .ownership(Ownership::Shared)
        .deadline(30_000_000_000)
        .liveliness(Liveliness::ManualByTopic, 2_000_000_000) // lease 2s
        .latency_budget(50_000_000)
        .transport_priority(8)
        .build()
        .unwrap()
}

/// QoS de teste para outputs: deadline de 1s nos DOIS lados.
fn outputs_qos_short_deadline() -> Qos {
    QosBuilder::new()
        .reliability(Reliability::Reliable, 10_000_000_000)
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(64))
        .ownership(Ownership::Exclusive)
        .deadline(1_000_000_000) // 1s
        .latency_budget(50_000_000)
        .transport_priority(8)
        .build()
        .unwrap()
}

fn make_agent(id: &str) -> AgentState {
    AgentState {
        agent_id: id.into(),
        hostname: "testhost".into(),
        model: "qwen".into(),
        specialization: "TEXT".into(),
        slots_total: 4,
        slots_busy: 0,
        vram_total_mb: 24000,
        vram_used_mb: 8000,
        ema_latency_ms: 10.0,
        completed_total: 0,
        failed_total: 0,
        health: 2,
        last_update_ns: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
        uptime_seconds: 1,
    }
}

fn make_output(tid: &str, seq: u32) -> TaskOutput {
    TaskOutput {
        task_id: tid.into(),
        seq_num: seq,
        content: "x".into(),
        is_final: false,
        finish_reason: 0,
        agent_id: "a".into(),
        token_count: 1,
        emitted_at_ns: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn liveliness_changed_fires_on_join_and_drop() {
    let ds_a = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let mon = QosMonitor::new();
    let mut rx = mon.subscribe();
    let qos = agents_qos_short_lease();
    // O Listener precisa sobreviver ao reader: o C chama os callbacks via ponteiro;
    // dropar o Listener cedo = use-after-free (SIGSEGV).
    let agents_listener = mon.agents_listener();
    let _reader = ds_a.agents_reader_with(&qos, &agents_listener);

    let ds_b = DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).unwrap();
    let writer_b = ds_b.agents_writer_with(&qos);

    tokio::time::sleep(Duration::from_millis(1500)).await; // settle/match

    // Writer aparece → alive +1
    writer_b.write(&make_agent("agent-live-1")).unwrap();
    writer_b.assert_liveliness().unwrap();

    let mut saw_join = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(ev) => {
                println!("[T-306] evento: {ev:?}");
                if let Ok(QosEvent::LivelinessChanged { alive_delta, .. }) = ev {
                    if alive_delta > 0 {
                        saw_join = true;
                        break;
                    }
                }
            }
            Err(_) => continue,
        }
    }
    assert!(saw_join, "liveliness changed (join) não disparou");

    // Writer "morre" à la SIGKILL: vaza o DataSpace SEM teardown (sem dispose/asserts)
    // → lease de 2s expira no reader → not_alive +1
    std::mem::forget(ds_b);
    std::mem::forget(writer_b);

    let mut saw_leave = false;
    for _ in 0..15 {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(ev) => {
                println!("[T-306] evento: {ev:?}");
                if let Ok(QosEvent::LivelinessChanged {
                    not_alive_delta, ..
                }) = ev
                {
                    if not_alive_delta > 0 {
                        saw_leave = true;
                        break;
                    }
                }
            }
            Err(_) => continue,
        }
    }
    assert!(
        saw_leave,
        "liveliness changed (leave/lease expirou) não disparou"
    );

    drop(_reader);
    ds_a.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requested_deadline_missed_detectado() {
    let ds_a = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let mon = QosMonitor::new();
    let mut rx = mon.subscribe();
    let qos = outputs_qos_short_deadline();
    let outputs_listener = mon.outputs_listener();
    let _reader = ds_a.outputs_reader_with(&qos, &outputs_listener);

    let ds_b = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let writer_b = ds_b.outputs_writer_with(&qos);

    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Escreve uma vez e PARA (deadline de 1s passa sem novas amostras)
    writer_b.write(&make_output("dl-task", 0)).unwrap();

    let mut saw_miss = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Ok(QosEvent::DeadlineMissed { delta, .. })) if delta > 0 => {
                saw_miss = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => continue,
        }
    }
    assert!(saw_miss, "requested deadline missed não disparou em ~1-3s");
    assert!(mon.deadlines_missed() > 0);

    drop(_reader);
    drop(writer_b);
    ds_a.shutdown().await.unwrap();
    ds_b.shutdown().await.unwrap();
}
