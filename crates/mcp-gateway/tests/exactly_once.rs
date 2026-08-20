mod common;

use common::TempDir;
use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::in_memory::InMemoryDataSpace;
use mcp_gateway::handler::ToolFuture;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::service::{status, ToolCallService};
use mcp_gateway::{FileClaimStore, OwnerId, ToolError, ToolHandler, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

struct AppendHandler(PathBuf);

struct FailAfterAppendHandler(PathBuf);

impl ToolHandler for AppendHandler {
    fn name(&self) -> &str {
        "test.append"
    }

    fn handle<'a>(&'a self, _arguments_json: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            use std::io::Write;
            let mut output = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.0)?;
            writeln!(output, "side-effect")?;
            Ok("appended".to_string())
        })
    }
}

impl ToolHandler for FailAfterAppendHandler {
    fn name(&self) -> &str {
        "test.fail_after_append"
    }

    fn handle<'a>(&'a self, _arguments_json: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            use std::io::Write;
            let mut output = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.0)?;
            writeln!(output, "side-effect-before-error")?;
            Err(ToolError::NotConfigured("intentional failure".into()))
        })
    }
}

fn registry(output: PathBuf) -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry.register(AppendHandler(output));
    registry
}

fn allowed_policy() -> Arc<DistributedPolicy> {
    let policy = Arc::new(DistributedPolicy::new("t807", Duration::from_secs(60)));
    let document = serde_json::json!({
        "version": 1,
        "rules": {
            "llm_inference": {
                "allowed_agents": ["agent-a"],
                "agent_policies": {
                    "agent-a": {"allowed_security_levels": ["PUBLIC"]}
                }
            },
            "tool_call": {
                "agent_tool_allowlist": {"agent-a": ["test.append", "test.fail_after_append"]},
                "high_risk_tools": [],
                "default_action": "DENY"
            }
        }
    });
    policy
        .ingest_snapshot(&SecurityPolicySnapshot {
            policy_id: "t807".into(),
            version: 1,
            policy_json: document.to_string(),
            published_by: "test".into(),
            timestamp_ns: now_ns(),
        })
        .expect("valid policy");
    policy
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn committed_error_and_restart_never_repeat_side_effect() {
    let temp = TempDir::new("claim-error-restart");
    let output = temp.path().join("effects.log");
    let claims_path = temp.path().join("claims");
    let request = ToolCallRequest {
        call_id: "failed-call".into(),
        request_id: "request".into(),
        requester_id: "agent-a".into(),
        tool_name: "test.fail_after_append".into(),
        arguments_json: "{}".into(),
        status: status::PENDING,
        created_at_ns: now_ns(),
        ..Default::default()
    };
    for owner in ["before-restart", "after-restart"] {
        let registry = ToolRegistry::new();
        registry.register(FailAfterAppendHandler(output.clone()));
        let service = ToolCallService::with_policy_and_claims(
            InMemoryDataSpace::new(),
            registry,
            allowed_policy(),
            Arc::new(FileClaimStore::new(&claims_path).expect("claims")),
            OwnerId::parse(owner).expect("owner"),
        );
        let _ = service.process_one(&request).await;
    }
    let effects = std::fs::read_to_string(output).expect("effect log");
    assert_eq!(effects.lines().count(), 1);
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_gateway_services_execute_one_external_side_effect() -> Result<(), ToolError> {
    let temp = TempDir::new("exactly-once-red");
    let output = temp.path().join("effects.log");
    let claims = Arc::new(FileClaimStore::new(&temp.path().join("claims")).expect("claims"));
    let policy = allowed_policy();
    let gateway_a = ToolCallService::with_policy_and_claims(
        InMemoryDataSpace::new(),
        registry(output.clone()),
        Arc::clone(&policy),
        claims.clone(),
        OwnerId::parse("gateway-a").expect("owner A"),
    );
    let gateway_b = ToolCallService::with_policy_and_claims(
        InMemoryDataSpace::new(),
        registry(output.clone()),
        policy,
        claims,
        OwnerId::parse("gateway-b").expect("owner B"),
    );
    let request = ToolCallRequest {
        call_id: "same-call".into(),
        request_id: "request".into(),
        requester_id: "agent-a".into(),
        tool_name: "test.append".into(),
        arguments_json: "{}".into(),
        status: status::PENDING,
        created_at_ns: now_ns(),
        ..Default::default()
    };

    let (a, b) = tokio::join!(
        gateway_a.process_one(&request),
        gateway_b.process_one(&request)
    );
    let completed = [a, b].into_iter().filter(Result::is_ok).count();
    assert_eq!(completed, 1, "one gateway must own the call");
    let effects = std::fs::read_to_string(output)?;
    assert_eq!(effects.lines().count(), 1, "duplicate external side effect");
    eprintln!("side_effects=1 duplicates=0 winners=1");
    Ok(())
}
