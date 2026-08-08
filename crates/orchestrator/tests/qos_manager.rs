//! Teste T-504: os 5 modos do `--qos-manager` rodam no loop de controle.
#![cfg(feature = "dds")]

use orch_common::FuzzyMetrics;
use orchestrator::dds::OrchestratorDds;
use qos_nfcm::decider::StaticDecider;
use qos_nfcm::fcm::{FcmDecider, FcmDhlDecider};
use qos_nfcm::zadeh::ZadehDecider;
use qos_nfcm::{Nfcm, QoSProfile};
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 104;

fn degraded() -> FuzzyMetrics {
    FuzzyMetrics {
        urgency: 0.60,
        deadline_pressure: 0.40,
        recent_latency: 0.85,
        agent_load: 0.80,
        error_rate: 0.90,
        historical_confidence: 0.20,
        estimated_complexity: 0.50,
        streaming_need: 0.10,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn t504_cada_modo_roda_no_control_loop() {
    let modes: Vec<(&str, Arc<dyn qos_nfcm::decider::QosDecider>, QoSProfile)> = vec![
        (
            "static",
            Arc::new(StaticDecider::new(QoSProfile::Balanced)),
            QoSProfile::Balanced,
        ),
        ("zadeh", Arc::new(ZadehDecider::new()), QoSProfile::Failover),
        ("fcm", Arc::new(FcmDecider::new()), QoSProfile::Failover),
        (
            "fcm-dhl",
            Arc::new(FcmDhlDecider::default()),
            QoSProfile::Failover,
        ),
        ("nfcm", Arc::new(Nfcm::qos_default()), QoSProfile::Failover),
    ];

    for (i, (mode, decider, expected)) in modes.into_iter().enumerate() {
        let orch = Arc::new(OrchestratorDds::new(DOMAIN + i as u32, decider, None).unwrap());
        orch.set_metrics(|m| *m = degraded());

        let d = orch.decide_once();
        assert_eq!(
            d.profile, expected,
            "modo {mode}: esperado {expected:?} no degradado, veio {:?}",
            d.profile
        );

        // Loop roda e decide periodicamente (cada modo)
        let _l = orch.spawn_control_loop(Duration::from_millis(200));
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(orch.decision_count() >= 2, "modo {mode}: loop não decidiu");
        _l.abort();
        println!(
            "[T-504] modo {mode}: OK ({:?}, conf {:.3})",
            d.profile, d.confidence
        );
    }
}
