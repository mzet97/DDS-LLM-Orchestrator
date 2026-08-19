#![cfg(feature = "dds")]

mod common;
#[path = "common/dds_policy.rs"]
mod dds_policy;

use std::sync::Arc;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::service::{status, ToolCallService};

use common::TempDir;
use dds_policy::{now_ns, policy_id, request, send_and_wait, snapshot, wait_until_allowed};

const DOMAIN: u32 = 108;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rejected_future_snapshot_does_not_poison_valid_recovery() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("mcp_gateway=info")
        .json()
        .with_test_writer()
        .try_init();
    let given_root = TempDir::new("dds-policy-recovery");
    let given_data_space =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("gateway starts");
    let given_policy = Arc::new(DistributedPolicy::new(policy_id(), Duration::from_secs(10)));
    let given_registry =
        mcp_gateway::dds::default_registry(given_root.path()).expect("registry starts");
    let given_service = Arc::new(ToolCallService::with_policy(
        given_data_space,
        given_registry,
        given_policy,
    ));
    let service_task = tokio::spawn(Arc::clone(&given_service).run());

    let client =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).expect("client DataSpace starts");
    let mut result_stream = client.subscribe_tool_calls();
    let (result_tx, mut results) = tokio::sync::mpsc::channel::<ToolCallRequest>(32);
    let collector_task = tokio::spawn(async move {
        while let Some(item) = std::future::poll_fn(|cx| result_stream.as_mut().poll_next(cx)).await
        {
            if result_tx.send(item).await.is_err() {
                break;
            }
        }
    });
    tokio::time::sleep(Duration::from_secs(1)).await;

    client
        .write_security_snapshot(snapshot(1, &["AgentA"]))
        .await
        .expect("baseline snapshot writes");
    let baseline = request("baseline", "AgentA", "baseline.txt", 0);
    wait_until_allowed(&given_service, &baseline, 1).await;
    let baseline_result = send_and_wait(given_service.data_space(), &mut results, baseline).await;
    assert_eq!(baseline_result.status, status::COMPLETED);
    assert_eq!(
        std::fs::read_to_string(given_root.path().join("baseline.txt"))
            .expect("baseline side effect"),
        "baseline"
    );

    client
        .write_security_snapshot(SecurityPolicySnapshot {
            timestamp_ns: now_ns().saturating_add(60_000_000_000),
            ..snapshot(2, &["FutureAgent"])
        })
        .await
        .expect("future snapshot writes");
    client
        .write_security_snapshot(SecurityPolicySnapshot {
            policy_json: "malformed-policy-secret-do-not-log".into(),
            ..snapshot(2, &["MalformedAgent"])
        })
        .await
        .expect("malformed snapshot writes");
    tokio::time::sleep(Duration::from_millis(150)).await;

    client
        .write_security_snapshot(snapshot(2, &["RecoveryAgent"]))
        .await
        .expect("recovery snapshot writes");
    let recovered = request("recovered-v2", "RecoveryAgent", "recovered-v2.txt", 0);
    wait_until_allowed(&given_service, &recovered, 2).await;
    let recovered_result = send_and_wait(given_service.data_space(), &mut results, recovered).await;
    assert_eq!(recovered_result.status, status::COMPLETED);
    assert_eq!(
        std::fs::read_to_string(given_root.path().join("recovered-v2.txt"))
            .expect("recovery side effect"),
        "recovered-v2"
    );

    service_task.abort();
    collector_task.abort();
}
