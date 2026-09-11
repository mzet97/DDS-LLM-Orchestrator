//! Real CycloneDDS acceptance scenario for T-808 / REQ-708.
#![cfg(feature = "dds")]

use cyclonedds::DomainParticipant;
use dds_contract::generated::dds_llm_orchestrator::SystemMetric;
use dds_contract::generated::orchestrator::ServerStatus;
use dds_contract::{topics, typenames};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const DOMAIN: u32 = 208;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_dds_discovers_eighteen_topics_and_observes_telemetry_samples() {
    let publisher = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let subscriber = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let observer = DomainParticipant::new(DOMAIN).unwrap();
    let mut metrics = subscriber.subscribe_system_metrics();
    let mut statuses = subscriber.subscribe_server_status();

    let metric = SystemMetric {
        metric_name: "cpu.utilization".into(),
        component_id: "agent-t808".into(),
        component_type: 1,
        value: 0.75,
        unit: "ratio".into(),
        timestamp_ns: 808,
    };
    let status = ServerStatus {
        server_id: "llama-t808".into(),
        slots_idle: 3,
        slots_processing: 1,
        model_loaded: "t808.gguf".into(),
        ready: true,
    };

    let publish = async {
        tokio::time::sleep(Duration::from_millis(800)).await;
        publisher.write_system_metric(metric.clone()).await.unwrap();
        publisher.write_server_status(status.clone()).await.unwrap();
    };
    let observe = async {
        let (observed_metric, observed_status) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(5), metrics.next()),
            tokio::time::timeout(Duration::from_secs(5), statuses.next())
        );
        let observed_metric = observed_metric.unwrap().unwrap();
        let observed_status = observed_status.unwrap().unwrap();
        (observed_metric, observed_status)
    };
    let (_, (observed_metric, observed_status)) = tokio::join!(publish, observe);
    assert_eq!(observed_metric.metric_name, "cpu.utilization");
    assert_eq!(observed_metric.value, 0.75);
    assert_eq!(observed_status.server_id, "llama-t808");
    assert!(observed_status.ready);

    let expected_types = BTreeMap::from([
        (topics::TASKS, typenames::TASK),
        (topics::AGENT_REGISTRY, typenames::AGENT_STATE),
        (topics::TASK_OUTPUT, typenames::TASK_OUTPUT),
        (topics::SYSTEM_METRICS, typenames::SYSTEM_METRIC),
        (topics::LLM_REQUEST, typenames::LLM_INFERENCE_REQUEST),
        (topics::LLM_RESULT, typenames::LLM_INFERENCE_RESULT),
        (topics::LLM_ERROR, typenames::LLM_INFERENCE_ERROR),
        (topics::SERVER_STATUS, typenames::SERVER_STATUS),
        (topics::QOS_ROUTING_PROFILE, typenames::QOS_ROUTING_PROFILE),
        (topics::CONTEXT_SNAPSHOT, typenames::CONTEXT_SNAPSHOT),
        (topics::CONTEXT_UPDATE, typenames::CONTEXT_UPDATE),
        (topics::TOOL_CALL_REQUEST, typenames::TOOL_CALL_REQUEST),
        (topics::EXECUTION_TRACE, typenames::EXECUTION_TRACE_EVENT),
        (
            topics::SECURITY_POLICY_SNAPSHOT,
            typenames::SECURITY_POLICY_SNAPSHOT,
        ),
        (
            topics::SECURITY_POLICY_UPDATE,
            typenames::SECURITY_POLICY_UPDATE,
        ),
        (topics::QOS_METRIC, typenames::QOS_METRIC),
        (topics::QOS_VIOLATION, typenames::QOS_VIOLATION),
        (topics::QOS_DISCOVERY, typenames::DISCOVERY_EVENT),
    ]);
    let canonical_names = topics::ALL.into_iter().collect::<BTreeSet<_>>();
    let mut discovered = BTreeMap::new();
    for _ in 0..50 {
        for sample in observer.discovered_topics().unwrap() {
            let name = sample.topic_name();
            if canonical_names.contains(name.as_str()) {
                discovered.insert(name, sample.type_name_value());
            }
        }
        if discovered.len() == 18 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(discovered.len(), 18, "discovered={discovered:#?}");
    assert_eq!(
        discovered,
        expected_types
            .into_iter()
            .map(|(name, type_name)| (name.to_owned(), type_name.to_owned()))
            .collect()
    );

    drop(metrics);
    drop(statuses);
    subscriber.shutdown().await.unwrap();
    publisher.shutdown().await.unwrap();
}
