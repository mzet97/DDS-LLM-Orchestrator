//! Bateria de contract tests `DataSpaceApi` (T-301/T-307).
//!
//! A MESMA bateria roda contra `InMemoryDataSpace` (mock) e — com
//! `--features dds` — contra o `DataSpace` real (T-307 A/B).
//! Rodar: `cargo test -p dds-dataspace` (mock)
//!        `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`

use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, SystemMetric, Task, TaskOutput, ToolCallRequest,
};
use dds_contract::generated::orchestrator::ServerStatus;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::in_memory::InMemoryDataSpace;
use futures::StreamExt;
use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Espera `f` retornar Some (até ~3s). Necessária no DataSpace real, onde o
/// cache é alimentado pelas streams (sem write-through) — read-after-write é
/// eventualmente consistente. No mock resolve na 1ª chamada.
async fn eventually<T, F, Fut>(mut f: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    for _ in 0..150 {
        if let Some(v) = f().await {
            return Some(v);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

pub fn make_task(id: &str) -> Task {
    Task {
        task_id: id.into(),
        client_id: "contract-client".into(),
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
        created_at_ns: now_ns(),
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: now_ns() + 60_000_000_000,
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

pub fn make_agent(id: &str) -> AgentState {
    AgentState {
        agent_id: id.into(),
        hostname: "testhost".into(),
        model: "qwen3.5-0.8b".into(),
        specialization: "TEXT".into(),
        slots_total: 4,
        slots_busy: 0,
        vram_total_mb: 24000,
        vram_used_mb: 8000,
        ema_latency_ms: 12.5,
        completed_total: 0,
        failed_total: 0,
        health: 2,
        last_update_ns: now_ns(),
        uptime_seconds: 60,
    }
}

pub fn make_output(task_id: &str, seq: u32, is_final: bool) -> TaskOutput {
    TaskOutput {
        task_id: task_id.into(),
        seq_num: seq,
        content: format!("chunk-{seq}"),
        is_final,
        finish_reason: if is_final { 1 } else { 0 },
        agent_id: "agent-1".into(),
        token_count: 1,
        emitted_at_ns: now_ns(),
    }
}

pub fn make_tool_call(id: &str) -> ToolCallRequest {
    ToolCallRequest {
        call_id: id.into(),
        request_id: "request-1".into(),
        tool_name: "filesystem.read_file".into(),
        arguments_json: "{\"path\":\"x.txt\"}".into(),
        security_level: 0,
        status: 0,
        result_json: String::new(),
        error_message: String::new(),
        created_at_ns: now_ns(),
        completed_at_ns: 0,
        ..Default::default()
    }
}

fn make_system_metric(metric_name: &str, component_id: &str, value: f32) -> SystemMetric {
    SystemMetric {
        metric_name: metric_name.to_owned(),
        component_id: component_id.to_owned(),
        component_type: 1,
        value,
        unit: "ms".into(),
        timestamp_ns: now_ns(),
    }
}

fn make_server_status(server_id: &str) -> ServerStatus {
    ServerStatus {
        server_id: server_id.to_owned(),
        slots_idle: 2,
        slots_processing: 0,
        model_loaded: "qwen3.5-0.8b".into(),
        ready: true,
    }
}

/// Bateria mínima de contrato (aceite T-301; base do A/B T-307).
pub async fn contract_battery(ds: &impl DataSpaceApi) {
    // No DataSpace real, os caches são alimentados APENAS pelas streams (visão do
    // mesh) — sem write-through. Assinamos tudo primeiro (o mock não é afetado).
    let mut f1 = ds.subscribe_tasks();
    let mut f2 = ds.subscribe_agent_states();
    let mut f3 = ds.subscribe_task_outputs();
    let mut f4 = ds.subscribe_tool_calls();
    let mut f5 = ds.subscribe_system_metrics();
    let mut f6 = ds.subscribe_server_statuses();
    let h1 = tokio::spawn(async move { while f1.next().await.is_some() {} });
    let h2 = tokio::spawn(async move { while f2.next().await.is_some() {} });
    let h3 = tokio::spawn(async move { while f3.next().await.is_some() {} });
    let h4 = tokio::spawn(async move { while f4.next().await.is_some() {} });
    let h5 = tokio::spawn(async move { while f5.next().await.is_some() {} });
    let h6 = tokio::spawn(async move { while f6.next().await.is_some() {} });

    // --- Tasks: write/read/all ---
    ds.write_task(make_task("task-a")).await.unwrap();
    ds.write_task(make_task("task-b")).await.unwrap();

    let t = eventually(|| async { ds.read_task("task-a").await.ok().flatten() })
        .await
        .expect("task-a existe");
    assert_eq!(t.task_id, "task-a");
    assert_eq!(t.client_id, "contract-client");
    assert_eq!(t.status, 0);
    assert!(t.created_at_ns > 0);

    assert!(ds.read_task("task-inexistente").await.unwrap().is_none());

    let tasks = eventually(|| async {
        let v = ds.all_tasks().await.unwrap();
        if v.len() >= 2 {
            Some(v)
        } else {
            None
        }
    })
    .await
    .expect("all_tasks com 2 tasks");
    let mut ids: Vec<String> = tasks.iter().map(|t| t.task_id.clone()).collect();
    ids.sort();
    assert_eq!(ids, ["task-a", "task-b"]);

    // --- Agents: write/read/all ---
    ds.write_agent_state(make_agent("agent-1")).await.unwrap();
    let a = eventually(|| async { ds.read_agent_state("agent-1").await.ok().flatten() })
        .await
        .expect("agent-1 existe");
    assert_eq!(a.agent_id, "agent-1");
    assert_eq!(a.health, 2);
    let agents = eventually(|| async {
        let v = ds.all_agents().await.unwrap();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    })
    .await
    .expect("all_agents não vazio");
    assert_eq!(agents.len(), 1);

    // --- Outputs: write + read por task ---
    for seq in 0..3 {
        ds.write_task_output(make_output("task-a", seq, seq == 2))
            .await
            .unwrap();
    }
    let outs = eventually(|| async {
        let v = ds.read_task_outputs("task-a").await.unwrap();
        if v.len() >= 3 {
            Some(v)
        } else {
            None
        }
    })
    .await
    .expect("3 outputs de task-a");
    assert_eq!(outs[0].seq_num, 0);
    assert!(outs[2].is_final);

    // --- subscribe_tasks: wakeup por amostra (sem polling) ---
    // Nota A/B: com TransientLocal (DDS real), o reader recebe o HISTÓRICO antes
    // das novas amostras; o mock (broadcast) só entrega as novas. O laço abaixo
    // aceita ambos: consome até a amostra nova chegar.
    let mut sub = ds.subscribe_tasks();
    ds.write_task(make_task("task-c")).await.unwrap();
    let mut got_c = None;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(2), sub.next()).await {
            Ok(Some(t)) if t.task_id == "task-c" => {
                got_c = Some(t);
                break;
            }
            Ok(Some(_)) => continue, // histórico TransientLocal
            _ => panic!("subscribe_tasks deveria acordar"),
        }
    }
    let got = got_c.expect("task-c recebida via subscribe");
    assert_eq!(got.client_id, "contract-client");

    // --- subscribe_task_outputs ---
    let mut sub_o = ds.subscribe_task_outputs();
    ds.write_task_output(make_output("task-a", 99, false))
        .await
        .unwrap();
    let mut got_99 = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(2), sub_o.next()).await {
            Ok(Some(o)) if o.seq_num == 99 => {
                got_99 = true;
                break;
            }
            Ok(Some(_)) => continue,
            _ => panic!("subscribe_task_outputs deveria acordar"),
        }
    }
    assert!(got_99, "output seq 99 não chegou via subscribe");

    ds.write_tool_call(make_tool_call("call-a")).await.unwrap();
    let call = eventually(|| async { ds.read_tool_call("call-a").await.unwrap() })
        .await
        .expect("ToolCallRequest call-a existe");
    assert_eq!(call.call_id, "call-a");
    assert_eq!(call.status, 0);

    ds.write_system_metric(make_system_metric("gpu_temp", "agent-1", 67.2))
        .await
        .unwrap();
    let metric = eventually(|| async {
        ds.read_system_metric("gpu_temp", "agent-1")
            .await
            .ok()
            .flatten()
    })
    .await
    .expect("system metric existe");
    assert_eq!(metric.metric_name, "gpu_temp");
    assert_eq!(metric.component_id, "agent-1");
    assert_eq!(metric.value, 67.2);

    let mut sys_sub = ds.subscribe_system_metrics();
    let qps_metric = make_system_metric("qps", "agent-2", 142.0);
    ds.write_system_metric(qps_metric.clone()).await.unwrap();
    let mut got_qps = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(2), sys_sub.next()).await {
            Ok(Some(m)) if m.metric_name == "qps" && m.component_id == "agent-2" => {
                got_qps = true;
                assert!((m.value - 142.0).abs() < f32::EPSILON);
                break;
            }
            Ok(Some(_)) => continue,
            _ => panic!("subscribe_system_metrics deveria receber métrica"),
        }
    }
    assert!(got_qps, "system metric recebida via subscribe");

    ds.write_server_status(make_server_status("llm-gpu-1"))
        .await
        .unwrap();
    let status = eventually(|| async { ds.read_server_status("llm-gpu-1").await.ok().flatten() })
        .await
        .expect("server status existe");
    assert_eq!(status.server_id, "llm-gpu-1");
    assert!(status.ready);

    let mut status_sub = ds.subscribe_server_statuses();
    let status = make_server_status("llm-gpu-2");
    ds.write_server_status(status).await.unwrap();
    let mut got_status2 = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_secs(2), status_sub.next()).await {
            Ok(Some(s)) if s.server_id == "llm-gpu-2" => {
                got_status2 = true;
                assert_eq!(s.slots_idle, 2);
                assert!(s.ready);
                break;
            }
            Ok(Some(_)) => continue,
            _ => panic!("subscribe_server_statuses deveria receber status"),
        }
    }
    assert!(got_status2, "server status recebido via subscribe");

    // --- shutdown ---
    ds.shutdown().await.unwrap();
    assert!(ds.all_tasks().await.unwrap().is_empty());

    h1.abort();
    h2.abort();
    h3.abort();
    h4.abort();
    h5.abort();
    h6.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_in_memory() {
    let ds = InMemoryDataSpace::new();
    contract_battery(&ds).await;
}

/// T-307: a MESMA bateria contra o DataSpace real (DDS).
#[cfg(feature = "dds")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_real_dds() {
    let ds = dds_dataspace::DataSpace::new(83, dds_dataspace::DataSpace::STRENGTH_ORCHESTRATOR)
        .expect("DataSpace sobe");
    contract_battery(&ds).await;
}
