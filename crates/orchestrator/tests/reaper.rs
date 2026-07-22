//! Teste do reaper (T-403): agente morre (heartbeat para) → suas tasks
//! ASSIGNED/RUNNING voltam para PENDING com retry_count+1.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p orchestrator --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use orchestrator::dds::OrchestratorDds;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 101;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_task(id: &str, agent: &str, status: i32) -> Task {
    Task {
        task_id: id.into(),
        client_id: "c".into(),
        assigned_agent: agent.into(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 8,
        stream: false,
        status,
        priority: 5,
        created_at_ns: now_ns(),
        assigned_at_ns: now_ns(),
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

fn make_agent(id: &str) -> AgentState {
    AgentState {
        agent_id: id.into(),
        hostname: "h".into(),
        model: "qwen".into(),
        specialization: "TEXT".into(),
        slots_total: 4,
        slots_busy: 1,
        vram_total_mb: 0,
        vram_used_mb: 0,
        ema_latency_ms: 0.0,
        completed_total: 0,
        failed_total: 0,
        health: 2,
        last_update_ns: now_ns(),
        uptime_seconds: 1,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t403_agente_morto_reatribui_tasks() {
    let orch =
        Arc::new(OrchestratorDds::new(DOMAIN, Arc::new(qos_nfcm::Nfcm::qos_default())).unwrap());
    let _feeders = orch.spawn_cache_feeders();
    let _mon = orch.spawn_registry_monitor(Duration::from_secs(2), Duration::from_millis(500));

    // Agente moribundo: aparece no registry (heartbeat) e claima uma task
    let ds_agent = DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).unwrap();
    ds_agent
        .write_agent_state(make_agent("agent-moribundo"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    ds_agent
        .write_task(make_task("reaper-task-1", "agent-moribundo", 1))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(800)).await;
    std::mem::forget(ds_agent); // SIGKILL: heartbeat para → staleness

    // Reaper (stale_after=2s, check 500ms) deve reatribuir para PENDING, retry=1
    let mut stream = Box::pin(orch.dataspace().stream_tasks());
    let mut reassigned = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), stream.next()).await {
            Ok(Some(t))
                if t.task_id == "reaper-task-1"
                    && t.status == 0
                    && t.retry_count == 1
                    && t.assigned_agent.is_empty() =>
            {
                reassigned = true;
                break;
            }
            Ok(Some(_)) => continue,
            _ => continue,
        }
    }

    assert!(
        reassigned,
        "reaper não reatribuiu a task do agente morto para PENDING"
    );
}

/// Regressão (Rodada 5, 2026-07-22): achado em produção real — um agente
/// travado (heartbeat parado, processo ainda vivo) ficou re-detectado como
/// morto pelo reaper A CADA CICLO (a cada `check_every`) por MAIS DE 2 HORAS
/// contínuas, porque `reap_dead_agents` nunca removia o agente de
/// `last_seen` — o mesmo timestamp obsoleto continuava batendo no filtro
/// `duration_since(...) > stale_after` para sempre. Efeito observável:
/// `QoS.Violation("liveliness_lost")` republicado (e o warn de log) em todo
/// ciclo, não só uma vez. Este teste prova que, com o fix (`last_seen.remove`
/// após reap), o agente morto gera EXATAMENTE 1 violação, não N.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t403b_agente_morto_nao_republica_violacao_a_cada_ciclo() {
    const DOMAIN_B: u32 = 106; // distinto de DOMAIN=101 acima, mesmo arquivo de teste
    let orch =
        Arc::new(OrchestratorDds::new(DOMAIN_B, Arc::new(qos_nfcm::Nfcm::qos_default())).unwrap());
    let _feeders = orch.spawn_cache_feeders();
    // Ciclos rápidos (stale_after=1s, check a cada 300ms) para observar
    // várias oportunidades de re-detecção dentro de poucos segundos de
    // teste — sem isto, levaria minutos para provar "não repete".
    let _mon = orch.spawn_registry_monitor(Duration::from_secs(1), Duration::from_millis(300));

    let mut violations = Box::pin(orch.dataspace().stream_qos_violations());

    let ds_agent = DataSpace::new(DOMAIN_B, DataSpace::STRENGTH_AGENT).unwrap();
    ds_agent
        .write_agent_state(make_agent("agent-repeticao"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    std::mem::forget(ds_agent); // heartbeat para

    // stale_after=1s + ~4s de observação ⇒ ~13 ciclos de reap (300ms) depois
    // que o agente vira "morto" — sem o fix, esperaríamos ~10+ violações
    // republicadas; com o fix, exatamente 1.
    let mut count = 0u32;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, violations.next()).await {
            Ok(Some(v)) if v.affected_entity == "agent-repeticao" => count += 1,
            Ok(Some(_)) => continue,
            _ => break,
        }
    }

    assert_eq!(
        count, 1,
        "esperava exatamente 1 QoS.Violation(liveliness_lost) para o agente morto, \
         não {count} — reaper voltou a republicar a cada ciclo (regressão do fix de last_seen)"
    );
}
