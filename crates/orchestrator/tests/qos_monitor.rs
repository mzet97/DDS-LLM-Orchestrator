//! Teste do porte de `QoSMonitor` (qos_monitor.py): task com deadline
//! expirado publica `QoS.Violation` + `QoS.Metric`; agente morto (reaper)
//! publica `QoS.Violation("liveliness_lost")`.
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use orchestrator::dds::OrchestratorDds;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 107;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_task(id: &str, status: i32, deadline_ns: u64) -> Task {
    Task {
        task_id: id.into(),
        client_id: "c".into(),
        assigned_agent: "agent-x".into(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 8,
        stream: false,
        status,
        priority: 5,
        created_at_ns: now_ns() - 1_000_000_000, // 1s atrás
        assigned_at_ns: now_ns(),
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn task_com_deadline_expirado_publica_violation_e_metric() {
    let orch =
        Arc::new(OrchestratorDds::new(DOMAIN, Arc::new(qos_nfcm::Nfcm::qos_default())).unwrap());
    let _feeders = orch.spawn_cache_feeders();

    let observer = DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).unwrap();
    let mut violations = Box::pin(observer.stream_qos_violations());
    let mut metrics = Box::pin(observer.stream_qos_metrics());

    // Task RUNNING (status=2, não-terminal) com deadline já vencido.
    let ds_agent = DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).unwrap();
    ds_agent
        .write_task(make_task("deadline-task-1", 2, now_ns().saturating_sub(1)))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await; // alimenta o cache

    let _qos_monitor = orch.spawn_qos_monitor(Duration::from_millis(200));

    let violation = tokio::time::timeout(Duration::from_secs(5), violations.next())
        .await
        .expect("timeout esperando QoS.Violation")
        .expect("stream fechou sem publicar");

    assert_eq!(violation.violation_type, "requested_deadline_missed");
    assert_eq!(violation.topic_name, "Tasks");
    assert_eq!(violation.entity_kind, "READER");
    assert_eq!(violation.affected_entity, "deadline-task-1");
    assert_eq!(violation.severity, "ERROR");
    assert_eq!(violation.component, "orchestrator");
    assert!(violation
        .violation_id
        .starts_with("requested_deadline_missed-Tasks-"));
    let details: serde_json::Value = serde_json::from_str(&violation.details_json).unwrap();
    assert_eq!(details["task_id"], "deadline-task-1");
    assert!(details["overdue_ms"].as_f64().unwrap() >= 0.0);

    let metric = tokio::time::timeout(Duration::from_secs(5), metrics.next())
        .await
        .expect("timeout esperando QoS.Metric")
        .expect("stream fechou sem publicar");
    assert_eq!(metric.metric_name, "requested_deadline_missed");
    assert_eq!(metric.metric_id, "orchestrator-requested_deadline_missed");
    assert_eq!(metric.value, 1);
    assert_eq!(metric.delta, 1);
    assert_eq!(metric.component, "orchestrator");

    // 2ª rodada do monitor não deve reportar a MESMA task de novo (dedup).
    let second = tokio::time::timeout(Duration::from_millis(500), violations.next()).await;
    assert!(
        second.is_err(),
        "não deveria republicar violation da mesma task (dedup, paridade com _reported_deadlines)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn agente_morto_publica_liveliness_lost_violation() {
    let orch = Arc::new(
        OrchestratorDds::new(DOMAIN + 1, Arc::new(qos_nfcm::Nfcm::qos_default())).unwrap(),
    );
    let _feeders = orch.spawn_cache_feeders();
    let _mon = orch.spawn_registry_monitor(Duration::from_secs(1), Duration::from_millis(300));

    let observer = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_CLIENT).unwrap();
    let mut violations = Box::pin(observer.stream_qos_violations());

    let ds_agent = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_AGENT).unwrap();
    ds_agent
        .write_agent_state(AgentState {
            agent_id: "agent-moribundo-qos".into(),
            hostname: "h".into(),
            model: "qwen".into(),
            specialization: "TEXT".into(),
            slots_total: 4,
            slots_busy: 0,
            vram_total_mb: 0,
            vram_used_mb: 0,
            ema_latency_ms: 0.0,
            completed_total: 0,
            failed_total: 0,
            health: 2,
            last_update_ns: now_ns(),
            uptime_seconds: 1,
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    std::mem::forget(ds_agent); // SIGKILL: heartbeat para → staleness

    let violation = tokio::time::timeout(Duration::from_secs(10), violations.next())
        .await
        .expect("timeout esperando QoS.Violation de liveliness_lost")
        .expect("stream fechou sem publicar");

    assert_eq!(violation.violation_type, "liveliness_lost");
    assert_eq!(violation.topic_name, "AgentRegistry");
    assert_eq!(violation.entity_kind, "WRITER");
    assert_eq!(violation.affected_entity, "agent-moribundo-qos");
    assert_eq!(violation.severity, "CRITICAL");
}
