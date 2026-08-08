//! Teste de `--fuzzy-routing`: publicação de `QoS.RoutingProfile` quando o
//! decisor de QoS troca de perfil (porte de
//! `test_task_consumer_fuzzy_routing.py`/`_publish_fuzzy_routing_profile`).
#![cfg(feature = "dds")]

use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use orch_common::FuzzyMetrics;
use orchestrator::dds::OrchestratorDds;
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 105;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fuzzy_routing_publica_qos_routing_profile_na_troca_de_perfil() {
    let orch = Arc::new(
        OrchestratorDds::new(
            DOMAIN,
            Arc::new(qos_nfcm::decider::StaticDecider::new(
                qos_nfcm::QoSProfile::Failover,
            )),
            None,
        )
        .unwrap()
        .with_fuzzy_routing(true),
    );

    // Observador independente no mesmo domínio, como os demais testes de
    // interop deste crate (ver control_loop.rs) — evita depender do cache
    // interno do próprio OrchestratorDds.
    let observer = DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).unwrap();
    let mut routing_stream = Box::pin(observer.stream_qos_routing());

    orch.set_metrics(|m| *m = FuzzyMetrics::default());

    let control_loop = orch.spawn_control_loop(Duration::from_millis(200));

    let profile = tokio::time::timeout(Duration::from_secs(5), routing_stream.next())
        .await
        .expect("timeout esperando QoS.RoutingProfile")
        .expect("stream fechou sem publicar");
    control_loop.abort();

    assert_eq!(profile.profile_id, "GLOBAL");
    assert_eq!(profile.profile_name, "QoS_Failover");
    assert_eq!(profile.version, 1, "1ª publicação deve ter version=1");
    assert_eq!(
        profile.preferred_agent_prefix, "",
        "perfis ponderados não definem preferido (paridade Python)"
    );
    assert_eq!(profile.fallback_after_ms, 300_000);

    let weights: serde_json::Value = serde_json::from_str(&profile.weights_json).unwrap();
    assert!(weights.is_object() && !weights.as_object().unwrap().is_empty());
    let explanation: serde_json::Value = serde_json::from_str(&profile.explanation_json).unwrap();
    assert_eq!(explanation["profile"], "QoS_Failover");
    assert_eq!(explanation["source"], "fuzzy_qos");

    // Perfil não mudou entre ciclos → não deve publicar de novo (dedup).
    let second = tokio::time::timeout(Duration::from_millis(500), routing_stream.next()).await;
    assert!(
        second.is_err(),
        "não deveria republicar o mesmo perfil (dedup, paridade com _last_routing_profile_name)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_fuzzy_routing_nao_publica_nada() {
    let orch = Arc::new(
        OrchestratorDds::new(
            DOMAIN + 1,
            Arc::new(qos_nfcm::decider::StaticDecider::new(
                qos_nfcm::QoSProfile::Balanced,
            )),
            None,
        )
        .unwrap(), // with_fuzzy_routing não chamado — default OFF
    );
    let observer = DataSpace::new(DOMAIN + 1, DataSpace::STRENGTH_CLIENT).unwrap();
    let mut routing_stream = Box::pin(observer.stream_qos_routing());

    let control_loop = orch.spawn_control_loop(Duration::from_millis(100));
    let result = tokio::time::timeout(Duration::from_millis(500), routing_stream.next()).await;
    control_loop.abort();

    assert!(
        result.is_err(),
        "com --fuzzy-routing desligado (default), nada deve ser publicado em QoS.RoutingProfile"
    );
}
