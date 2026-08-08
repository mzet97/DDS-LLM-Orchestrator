//! Teste do loop de controle com NFCM (T-405): cenário degradado → Failover;
//! knobs online aplicados sem erro (com DDS real).
#![cfg(feature = "dds")]

use orch_common::FuzzyMetrics;
use orchestrator::dds::OrchestratorDds;
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 100;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t405_degradado_seleciona_failover_e_aplica_knobs() {
    let orch = Arc::new(
        OrchestratorDds::new(DOMAIN, Arc::new(qos_nfcm::Nfcm::qos_default()), None).unwrap(),
    );

    // Cenário degradado canônico (mesmo do teste do artigo no qos-nfcm):
    // error_rate=0.90 alto → esperado QoS_Failover com μ_alto≈0.923
    orch.set_metrics(|m| {
        *m = FuzzyMetrics {
            urgency: 0.60,
            deadline_pressure: 0.40,
            recent_latency: 0.85,
            agent_load: 0.80,
            error_rate: 0.90,
            historical_confidence: 0.20,
            estimated_complexity: 0.50,
            streaming_need: 0.10,
        };
    });

    let result = orch.decide_once();
    assert_eq!(
        result.profile,
        qos_nfcm::QoSProfile::Failover,
        "degradado deve selecionar Failover"
    );

    // Aplica os knobs do perfil decidido no writer de Tasks (set_qos real)
    let (_s, knobs) = dds_contract::qos_profile("QoS_Failover").unwrap();
    orch.dataspace()
        .apply_tasks_knobs(&knobs)
        .expect("aplicar knobs");

    // Loop rodando: N decisões tracejadas
    let _loop = orch.spawn_control_loop(Duration::from_millis(300));
    tokio::time::sleep(Duration::from_millis(1000)).await;
    let n = orch.decision_count();
    assert!(n >= 2, "esperava ≥2 decisões do loop, teve {n}");
    _loop.abort();

    println!(
        "[T-405] degradado → {:?} ({} decisões no loop)",
        result.profile, n
    );
}

/// Regressão (Rodada 8, 2026-07-22): antes desta fiação, `set_metrics` não
/// tinha NENHUM chamador em produção — o control loop decidia sobre
/// `FuzzyMetrics` constantes em todo ciclo e qualquer braço adaptativo
/// (`--qos-manager nfcm/zadeh/fcm/fcm-dhl`) degenerava em braço estático,
/// invalidando a campanha comparativa do artigo (§9). Este teste prova que
/// `refresh_metrics_from_mesh()` (porte de `_collect_fuzzy_metrics` do
/// Python) produz métricas REAIS a partir do estado do mesh (caches de
/// AgentRegistry + Tasks), não os defaults.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rodada8_refresh_metrics_le_o_mesh_nao_zeros() {
    use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task};
    const DOMAIN_B: u32 = 108; // distinto do DOMAIN=100 do teste acima

    let orch = Arc::new(
        OrchestratorDds::new(DOMAIN_B, Arc::new(qos_nfcm::Nfcm::qos_default()), None).unwrap(),
    );

    // Semeia o cache diretamente (refresh lê caches, não o wire — o feeder
    // de produção os alimenta via stream_tasks/stream_agent_states).
    let caches = orch.dataspace().caches();
    caches.upsert_agent(AgentState {
        agent_id: "m-agent".into(),
        hostname: "h".into(),
        model: "qwen".into(),
        specialization: "TEXT".into(),
        slots_total: 4,
        slots_busy: 3,
        vram_total_mb: 0,
        vram_used_mb: 0,
        ema_latency_ms: 500.0,
        completed_total: 90,
        failed_total: 10,
        health: 2,
        last_update_ns: 1,
        uptime_seconds: 1,
    });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    for (i, status) in [(0u32, 0i32), (1, 2), (2, 3)] {
        caches.upsert_task(Task {
            task_id: format!("m-task-{i}"),
            client_id: "c".into(),
            assigned_agent: String::new(),
            target_agent: String::new(),
            model_required: 0,
            model_name: "qwen".into(),
            messages_json: "x".repeat(2000),
            temperature: 0.7,
            max_tokens: 8,
            stream: i == 0,
            status,
            priority: 5,
            created_at_ns: now,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: if status == 3 { now } else { 0 },
            deadline_ns: now + 60_000_000_000,
            retry_count: 0,
            finish_reason: String::new(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        });
    }

    orch.refresh_metrics_from_mesh();

    let mut got = FuzzyMetrics::default();
    orch.set_metrics(|m| got = *m);

    // agent_load = 3/4; recent_latency = 500/1000; error_rate = 10/100;
    // historical_confidence = 90/100; urgency = 2 ativas (PENDING+RUNNING)/3;
    // streaming_need = 1/2; estimated_complexity = 2000/4000.
    assert!(
        (got.agent_load - 0.75).abs() < 1e-9,
        "agent_load={}",
        got.agent_load
    );
    assert!(
        (got.recent_latency - 0.5).abs() < 1e-9,
        "recent_latency={}",
        got.recent_latency
    );
    assert!(
        (got.error_rate - 0.10).abs() < 1e-9,
        "error_rate={}",
        got.error_rate
    );
    assert!(
        (got.historical_confidence - 0.90).abs() < 1e-9,
        "historical_confidence={}",
        got.historical_confidence
    );
    assert!(
        (got.urgency - 2.0 / 3.0).abs() < 1e-9,
        "urgency={}",
        got.urgency
    );
    assert!(
        (got.streaming_need - 0.5).abs() < 1e-9,
        "streaming_need={}",
        got.streaming_need
    );
    assert!(
        (got.estimated_complexity - 0.5).abs() < 1e-9,
        "estimated_complexity={}",
        got.estimated_complexity
    );
    assert!(
        got.deadline_pressure.abs() < 1e-9,
        "deadline_pressure={}",
        got.deadline_pressure
    );

    println!("[Rodada 8] refresh_metrics_from_mesh: 8 métricas coletadas do mesh corretamente");
}
