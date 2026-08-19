#![cfg(feature = "dds")]

mod common;
#[path = "common/dds_policy.rs"]
mod dds_policy;

use std::sync::Arc;
use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::SecurityPolicyUpdate;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::service::{status, ToolCallService};
use mcp_gateway::tools::FilesystemTool;

use common::TempDir;
use dds_policy::{now_ns, policy_id, request, send_and_wait, snapshot, wait_until_allowed};

const DOMAIN: u32 = 106;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn policy_lifecycle_controls_real_dds_filesystem_side_effects() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("mcp_gateway=info")
        .json()
        .with_test_writer()
        .try_init();
    let given_root = TempDir::new("dds-policy");
    let given_data_space =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("gateway DataSpace starts");
    let given_policy = Arc::new(DistributedPolicy::new(
        policy_id(),
        Duration::from_millis(750),
    ));
    let given_registry =
        mcp_gateway::dds::default_registry(given_root.path()).expect("filesystem registry starts");
    let given_service = Arc::new(ToolCallService::with_policy(
        given_data_space,
        given_registry,
        given_policy,
    ));
    let service_task = tokio::spawn(Arc::clone(&given_service).run());

    let client =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).expect("client DataSpace starts");
    let mut result_stream = client.subscribe_tool_calls();
    let (result_tx, mut results) = tokio::sync::mpsc::channel(64);
    let collector_task = tokio::spawn(async move {
        while let Some(item) = std::future::poll_fn(|cx| result_stream.as_mut().poll_next(cx)).await
        {
            if result_tx.send(item).await.is_err() {
                break;
            }
        }
    });
    tokio::time::sleep(Duration::from_secs(1)).await;

    let no_snapshot = request("no-snapshot", "AgentA", "no-snapshot.txt", 0);
    let no_snapshot_result =
        send_and_wait(given_service.data_space(), &mut results, no_snapshot).await;
    assert_eq!(no_snapshot_result.status, status::DENIED);
    assert!(!given_root.path().join("no-snapshot.txt").exists());

    client
        .write_security_snapshot(snapshot(1, &["AgentA"]))
        .await
        .expect("snapshot writes over DDS");
    let valid = request("valid", "AgentA", "valid.txt", 0);
    wait_until_allowed(&given_service, &valid, 1).await;
    let valid_result = send_and_wait(given_service.data_space(), &mut results, valid).await;
    assert_eq!(valid_result.status, status::COMPLETED);
    assert_eq!(
        std::fs::read_to_string(given_root.path().join("valid.txt")).expect("valid side effect"),
        "valid"
    );

    for (call_id, identity, path, level) in [
        (
            "wrong-identity",
            "requester-secret-do-not-log",
            "wrong-identity.txt",
            0,
        ),
        ("level-minus-one", "AgentA", "minus-one.txt", -1),
        ("level-four", "AgentA", "four.txt", 4),
    ] {
        let terminal = send_and_wait(
            given_service.data_space(),
            &mut results,
            request(call_id, identity, path, level),
        )
        .await;
        assert_eq!(terminal.status, status::DENIED, "{call_id}");
        assert!(!given_root.path().join(path).exists(), "{call_id}");
    }

    tokio::time::sleep(Duration::from_millis(800)).await;
    let expired = request("expired", "AgentA", "expired.txt", 0);
    let expired_result = send_and_wait(given_service.data_space(), &mut results, expired).await;
    assert_eq!(expired_result.status, status::DENIED);
    assert!(!given_root.path().join("expired.txt").exists());

    client
        .write_security_snapshot(snapshot(2, &["AgentA"]))
        .await
        .expect("fresh snapshot writes over DDS");
    let fresh = request("fresh-v2", "AgentA", "fresh-v2.txt", 0);
    wait_until_allowed(&given_service, &fresh, 2).await;
    let update = SecurityPolicyUpdate {
        policy_id: policy_id(),
        previous_version: 2,
        new_version: 3,
        operation: "UPDATE_RULE".into(),
        rule_delta_json: serde_json::json!({
            "rules": {
                "llm_inference": {
                    "allowed_agents": ["AgentA", "AgentB"],
                    "agent_policies": {
                        "AgentB": {"allowed_security_levels": ["PUBLIC"]}
                    }
                },
                "tool_call": {
                    "agent_tool_allowlist": {
                        "AgentB": [FilesystemTool::WRITE_FILE]
                    }
                }
            }
        })
        .to_string(),
        published_by: "policy-engine-v1".into(),
        timestamp_ns: now_ns(),
    };
    client
        .write_security_update(update)
        .await
        .expect("update writes over DDS");
    let updated = request("updated-v3", "AgentB", "updated-v3.txt", 0);
    wait_until_allowed(&given_service, &updated, 3).await;
    let updated_result = send_and_wait(given_service.data_space(), &mut results, updated).await;
    assert_eq!(updated_result.status, status::COMPLETED);
    assert!(given_root.path().join("updated-v3.txt").exists());

    client
        .write_security_snapshot(snapshot(1, &["AgentC"]))
        .await
        .expect("rollback snapshot writes over DDS");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let rollback = request("rollback", "AgentC", "rollback.txt", 0);
    let rollback_result = send_and_wait(given_service.data_space(), &mut results, rollback).await;
    assert_eq!(rollback_result.status, status::DENIED);
    assert!(!given_root.path().join("rollback.txt").exists());

    service_task.abort();
    collector_task.abort();
}
