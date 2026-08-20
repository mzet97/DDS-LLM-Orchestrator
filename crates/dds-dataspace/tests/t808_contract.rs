//! Acceptance contract for T-808 / REQ-708.

use dds_contract::generated::dds_llm_orchestrator::SystemMetric;
use dds_contract::generated::orchestrator::ServerStatus;
use dds_contract::topics;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::in_memory::InMemoryDataSpace;
use futures::StreamExt;
use std::time::Duration;

#[test]
fn canonical_topic_inventory_has_exactly_eighteen_unique_names() {
    let mut unique = topics::ALL.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(topics::ALL.len(), 18);
    assert_eq!(unique.len(), 18);
}

#[tokio::test]
async fn public_api_roundtrips_system_metrics_and_server_status() {
    let data_space = InMemoryDataSpace::new();
    let mut metrics = data_space.subscribe_system_metrics();
    let mut statuses = data_space.subscribe_server_status();
    let metric = SystemMetric {
        metric_name: "cpu.utilization".into(),
        component_id: "agent-1".into(),
        component_type: 1,
        value: 0.25,
        unit: "ratio".into(),
        timestamp_ns: 42,
    };
    let status = ServerStatus {
        server_id: "llama-1".into(),
        slots_idle: 2,
        slots_processing: 1,
        model_loaded: "model.gguf".into(),
        ready: true,
    };

    data_space
        .write_system_metric(metric.clone())
        .await
        .unwrap();
    data_space
        .write_server_status(status.clone())
        .await
        .unwrap();

    let observed_metric = tokio::time::timeout(Duration::from_secs(1), metrics.next())
        .await
        .unwrap()
        .unwrap();
    let observed_status = tokio::time::timeout(Duration::from_secs(1), statuses.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed_metric.metric_name, metric.metric_name);
    assert_eq!(observed_metric.component_id, metric.component_id);
    assert_eq!(observed_status.server_id, status.server_id);
    assert_eq!(observed_status.ready, status.ready);
}
