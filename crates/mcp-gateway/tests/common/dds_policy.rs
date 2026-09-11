use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use mcp_gateway::policy::PolicyDecision;
use mcp_gateway::service::{status, ToolCallService};
use mcp_gateway::tools::FilesystemTool;

pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

pub fn policy_id() -> String {
    format!("t806-policy-{}", std::process::id())
}

fn document(version: i32, identities: &[&str]) -> String {
    let tool_allowlist = identities
        .iter()
        .map(|identity| {
            (
                (*identity).to_string(),
                serde_json::json!([FilesystemTool::WRITE_FILE]),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let agent_policies = identities
        .iter()
        .map(|identity| {
            (
                (*identity).to_string(),
                serde_json::json!({"allowed_security_levels": ["PUBLIC"]}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({
        "version": version,
        "rules": {
            "llm_inference": {
                "allowed_agents": identities,
                "agent_policies": agent_policies
            },
            "tool_call": {
                "agent_tool_allowlist": tool_allowlist,
                "high_risk_tools": [],
                "default_action": "DENY"
            }
        }
    })
    .to_string()
}

pub fn snapshot(version: i32, identities: &[&str]) -> SecurityPolicySnapshot {
    SecurityPolicySnapshot {
        policy_id: policy_id(),
        version,
        policy_json: document(version, identities),
        published_by: "policy-engine-v1".into(),
        timestamp_ns: now_ns(),
    }
}

pub fn request(call_id: &str, identity: &str, path: &str, level: i32) -> ToolCallRequest {
    // A DDS domain can be shared by concurrent worktrees; correlate only this test process.
    let content = call_id;
    let call_id = format!("{call_id}-{}", std::process::id());
    ToolCallRequest {
        call_id: call_id.clone(),
        request_id: format!("correlation-{call_id}"),
        requester_id: identity.into(),
        tool_name: FilesystemTool::WRITE_FILE.into(),
        arguments_json: serde_json::json!({"path": path, "content": content}).to_string(),
        security_level: level,
        status: status::PENDING,
        created_at_ns: now_ns(),
        ..Default::default()
    }
}

pub async fn send_and_wait(
    gateway: &DataSpace,
    results: &mut tokio::sync::mpsc::Receiver<ToolCallRequest>,
    call: ToolCallRequest,
) -> ToolCallRequest {
    gateway
        .write_tool_call(call.clone())
        .await
        .expect("request writes over DDS");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let item = results.recv().await.expect("result collector stays open");
            if item.call_id == call.call_id
                && matches!(
                    item.status,
                    status::DENIED | status::COMPLETED | status::FAILED
                )
            {
                return item;
            }
        }
    })
    .await
    .expect("terminal result over DDS")
}

pub async fn wait_until_allowed(
    service: &ToolCallService<DataSpace>,
    call: &ToolCallRequest,
    version: i32,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if service.policy().evaluate(call) == (PolicyDecision::Allowed { version }) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("policy becomes active over DDS");
}
