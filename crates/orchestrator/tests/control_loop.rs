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
    let orch =
        Arc::new(OrchestratorDds::new(DOMAIN, Arc::new(qos_nfcm::Nfcm::qos_default())).unwrap());

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
