#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::{api::DataSpaceApi, DataSpace};
use futures::StreamExt;
use mcp_gateway::handler::ToolHandler;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::{MemoryClaimStore, OwnerId, ToolCallService, ToolRegistry};
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 95;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_tool_call(id: usize) -> ToolCallRequest {
    ToolCallRequest {
        call_id: format!("tool-call-{id}"),
        request_id: format!("request-{id}"),
        requester_id: "agent-a".into(),
        tool_name: "echo.tool".into(),
        arguments_json: format!("{{\"i\":{id}}}"),
        security_level: 0,
        status: 0,
        created_at_ns: now_ns(),
        ..Default::default()
    }
}

fn allowed_policy() -> Arc<DistributedPolicy> {
    let policy = Arc::new(DistributedPolicy::new("claim-dds", Duration::from_secs(60)));
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
                "agent_tool_allowlist": {"agent-a": ["echo.tool"]},
                "high_risk_tools": [],
                "default_action": "DENY"
            }
        }
    });
    policy
        .ingest_snapshot(&SecurityPolicySnapshot {
            policy_id: "claim-dds".into(),
            version: 1,
            policy_json: document.to_string(),
            published_by: "test".into(),
            timestamp_ns: now_ns(),
        })
        .expect("valid policy");
    policy
}

#[derive(Default)]
struct EchoTool(Arc<AtomicUsize>);

impl ToolHandler for EchoTool {
    fn name(&self) -> &str {
        "echo.tool"
    }

    fn handle<'a>(
        &'a self,
        arguments_json: &'a str,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<String, mcp_gateway::ToolError>> + Send + 'a>,
    > {
        self.0.fetch_add(1, Ordering::SeqCst);
        let text = arguments_json.to_string();
        Box::pin(async move { Ok(text) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn claim_prevents_duplicate_execution_with_two_gateways() {
    let exec_count = Arc::new(AtomicUsize::new(0));

    let data_space_one = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("ds-1");
    let data_space_two = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("ds-2");
    let producer = DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).expect("producer");
    let observer = DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).expect("observer");

    let registry_one = {
        let registry = ToolRegistry::new();
        registry.register_arc(Arc::new(EchoTool(exec_count.clone())));
        registry
    };

    let registry_two = {
        let registry = ToolRegistry::new();
        registry.register_arc(Arc::new(EchoTool(exec_count.clone())));
        registry
    };

    let policy = allowed_policy();
    let service_one = Arc::new(ToolCallService::with_policy_and_claims(
        data_space_one,
        registry_one,
        Arc::clone(&policy),
        Arc::new(MemoryClaimStore::default()),
        OwnerId::parse("claim-gw-1").expect("owner 1"),
    ));
    let service_two = Arc::new(ToolCallService::with_policy_and_claims(
        data_space_two,
        registry_two,
        policy,
        Arc::new(MemoryClaimStore::default()),
        OwnerId::parse("claim-gw-2").expect("owner 2"),
    ));

    let run_one = tokio::spawn({
        let service_one = Arc::clone(&service_one);
        async move { service_one.run().await.expect("gateway-1") }
    });
    let run_two = tokio::spawn({
        let service_two = Arc::clone(&service_two);
        async move { service_two.run().await.expect("gateway-2") }
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut stream = Box::pin(observer.subscribe_tool_calls());
    let total = 100usize;
    for i in 0..total {
        producer
            .write_tool_call(make_tool_call(i))
            .await
            .expect("escreve tool call");
    }

    let mut seen = HashSet::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while seen.len() < total {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "timeout esperando concluir tool calls"
        );
        let call = tokio::time::timeout(remaining, stream.next())
            .await
            .expect("timeout")
            .expect("stream");
        if call.status == 4 && seen.insert(call.call_id) {
            assert_eq!(call.error_message, "");
        }
    }

    run_one.abort();
    run_two.abort();
    assert_eq!(exec_count.load(Ordering::SeqCst), total);
    producer.shutdown().await.unwrap();
    observer.shutdown().await.unwrap();
}
