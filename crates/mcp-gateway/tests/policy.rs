use std::time::Duration;

use dds_contract::generated::dds_llm_orchestrator::{
    SecurityPolicySnapshot, SecurityPolicyUpdate, ToolCallRequest,
};
use mcp_gateway::policy::{DenialReason, DistributedPolicy, PolicyDecision, PolicyIngestError};

const NOW: u64 = 1_000_000;

fn document(version: i32, identity: &str, tool: &str) -> String {
    serde_json::json!({
        "version": version,
        "rules": {
            "llm_inference": {
                "allowed_agents": [identity],
                "agent_policies": {
                    identity: {"allowed_security_levels": ["PUBLIC", "INTERNAL"]}
                }
            },
            "tool_call": {
                "agent_tool_allowlist": {identity: [tool]},
                "high_risk_tools": [],
                "default_action": "DENY"
            }
        }
    })
    .to_string()
}

fn snapshot(version: i32, timestamp_ns: u64) -> SecurityPolicySnapshot {
    SecurityPolicySnapshot {
        policy_id: "default".into(),
        version,
        policy_json: document(version, "AgentA", "filesystem.read_file"),
        published_by: "policy-engine-v1".into(),
        timestamp_ns,
    }
}

fn request(identity: &str, tool: &str, level: i32) -> ToolCallRequest {
    ToolCallRequest {
        call_id: "call-1".into(),
        request_id: "correlation-1".into(),
        requester_id: identity.into(),
        tool_name: tool.into(),
        security_level: level,
        ..Default::default()
    }
}

#[test]
fn denies_without_snapshot_missing_requester_and_invalid_levels() {
    let given_policy = DistributedPolicy::new("default", Duration::from_nanos(100));

    let when_no_snapshot =
        given_policy.evaluate_at(&request("AgentA", "filesystem.read_file", 0), NOW);
    let when_missing_requester =
        given_policy.evaluate_at(&request("", "filesystem.read_file", 0), NOW);
    let when_minus_one =
        given_policy.evaluate_at(&request("AgentA", "filesystem.read_file", -1), NOW);
    let when_four = given_policy.evaluate_at(&request("AgentA", "filesystem.read_file", 4), NOW);

    assert_eq!(
        when_no_snapshot,
        PolicyDecision::Denied {
            reason: DenialReason::NoSnapshot
        }
    );
    assert_eq!(
        when_missing_requester,
        PolicyDecision::Denied {
            reason: DenialReason::MissingRequester
        }
    );
    assert_eq!(
        when_minus_one,
        PolicyDecision::Denied {
            reason: DenialReason::InvalidLevel
        }
    );
    assert_eq!(
        when_four,
        PolicyDecision::Denied {
            reason: DenialReason::InvalidLevel
        }
    );
}

#[test]
fn valid_snapshot_binds_requester_tool_and_level_then_expires() {
    let given_policy = DistributedPolicy::new("default", Duration::from_nanos(100));
    given_policy
        .ingest_snapshot_at(&snapshot(1, NOW - 10), NOW)
        .expect("valid snapshot");

    let when_exact = given_policy.evaluate_at(&request("AgentA", "filesystem.read_file", 1), NOW);
    let when_wrong_identity =
        given_policy.evaluate_at(&request("AgentB", "filesystem.read_file", 1), NOW);
    let when_wrong_tool =
        given_policy.evaluate_at(&request("AgentA", "filesystem.write_file", 1), NOW);
    let when_wrong_level =
        given_policy.evaluate_at(&request("AgentA", "filesystem.read_file", 2), NOW);
    let when_expired =
        given_policy.evaluate_at(&request("AgentA", "filesystem.read_file", 1), NOW + 101);

    assert_eq!(when_exact, PolicyDecision::Allowed { version: 1 });
    assert_eq!(
        when_wrong_identity,
        PolicyDecision::Denied {
            reason: DenialReason::ToolDenied
        }
    );
    assert_eq!(
        when_wrong_tool,
        PolicyDecision::Denied {
            reason: DenialReason::ToolDenied
        }
    );
    assert_eq!(
        when_wrong_level,
        PolicyDecision::Denied {
            reason: DenialReason::LevelDenied
        }
    );
    assert_eq!(
        when_expired,
        PolicyDecision::Denied {
            reason: DenialReason::Expired
        }
    );
}

#[test]
fn rejects_malformed_future_stale_and_conflicting_snapshots() {
    let given_policy = DistributedPolicy::new("default", Duration::from_nanos(100));
    let mut malformed = snapshot(1, NOW);
    malformed.policy_json = "not-json".into();
    let mut mismatch = snapshot(1, NOW);
    mismatch.policy_json = document(2, "AgentA", "filesystem.read_file");

    assert!(matches!(
        given_policy.ingest_snapshot_at(&snapshot(1, 0), NOW),
        Err(PolicyIngestError::InvalidTimestamp)
    ));
    assert!(matches!(
        given_policy.ingest_snapshot_at(&snapshot(1, NOW + 1), NOW),
        Err(PolicyIngestError::InvalidTimestamp)
    ));
    assert!(matches!(
        given_policy.ingest_snapshot_at(&snapshot(1, NOW - 101), NOW),
        Err(PolicyIngestError::InvalidTimestamp)
    ));
    assert!(matches!(
        given_policy.ingest_snapshot_at(&malformed, NOW),
        Err(PolicyIngestError::InvalidDocument(_))
    ));
    assert!(matches!(
        given_policy.ingest_snapshot_at(&mismatch, NOW),
        Err(PolicyIngestError::VersionMismatch)
    ));

    given_policy
        .ingest_snapshot_at(&snapshot(2, NOW - 1), NOW)
        .expect("new snapshot");
    assert!(matches!(
        given_policy.ingest_snapshot_at(&snapshot(1, NOW), NOW),
        Err(PolicyIngestError::StaleVersion)
    ));
    let mut conflict = snapshot(2, NOW);
    conflict.policy_json = document(2, "AgentB", "filesystem.write_file");
    assert!(matches!(
        given_policy.ingest_snapshot_at(&conflict, NOW),
        Err(PolicyIngestError::StaleVersion)
    ));
}

#[test]
fn monotonic_update_changes_effective_identity_and_rejects_rollback() {
    let given_policy = DistributedPolicy::new("default", Duration::from_nanos(100));
    given_policy
        .ingest_snapshot_at(&snapshot(1, NOW - 10), NOW)
        .expect("initial snapshot");
    let update = SecurityPolicyUpdate {
        policy_id: "default".into(),
        previous_version: 1,
        new_version: 2,
        operation: "UPDATE_RULE".into(),
        rule_delta_json: serde_json::json!({
            "rules": {
                "llm_inference": {
                    "allowed_agents": ["AgentB"],
                    "agent_policies": {
                        "AgentB": {"allowed_security_levels": ["PUBLIC"]}
                    }
                },
                "tool_call": {
                    "agent_tool_allowlist": {
                        "AgentB": ["filesystem.write_file"]
                    }
                }
            }
        })
        .to_string(),
        published_by: "policy-engine-v1".into(),
        timestamp_ns: NOW,
    };

    given_policy
        .ingest_update_at(&update, NOW)
        .expect("monotonic update");
    assert_eq!(
        given_policy.evaluate_at(&request("AgentB", "filesystem.write_file", 0), NOW,),
        PolicyDecision::Allowed { version: 2 }
    );

    let rollback = SecurityPolicyUpdate {
        previous_version: 1,
        new_version: 1,
        timestamp_ns: NOW,
        ..update
    };
    assert!(matches!(
        given_policy.ingest_update_at(&rollback, NOW),
        Err(PolicyIngestError::InvalidUpdateChain)
    ));
}
