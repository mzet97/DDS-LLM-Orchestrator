#![cfg(feature = "dds")]

mod common;

use common::TempDir;
use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use mcp_gateway::handler::ToolFuture;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::service::{status, ToolCallService};
use mcp_gateway::{
    ClaimDecision, ClaimError, ClaimStore, FileClaimStore, OwnerId, ToolHandler, ToolRegistry,
};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DOMAIN: u32 = 207;
const CALL_COUNT: usize = 100;

struct ObservedClaims {
    inner: Arc<FileClaimStore>,
    attempts: AtomicUsize,
    delay: Duration,
    loses_every_claim: bool,
}

impl ClaimStore for ObservedClaims {
    fn try_claim(&self, call_id: &str, owner: &OwnerId) -> Result<ClaimDecision, ClaimError> {
        self.attempts.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(self.delay);
        if self.loses_every_claim {
            return Ok(ClaimDecision::AlreadyClaimed);
        }
        self.inner.try_claim(call_id, owner)
    }
}

struct AppendHandler(PathBuf);

impl ToolHandler for AppendHandler {
    fn name(&self) -> &str {
        "test.append"
    }

    fn handle<'a>(&'a self, arguments_json: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let mut output = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.0)?;
            writeln!(output, "{arguments_json}")?;
            output.sync_all()?;
            Ok("appended".to_owned())
        })
    }
}

fn registry(output: PathBuf) -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry.register(AppendHandler(output));
    registry
}

fn policy() -> Arc<DistributedPolicy> {
    let policy = Arc::new(DistributedPolicy::new("t807-dds", Duration::from_secs(60)));
    let document = serde_json::json!({
        "version": 1,
        "rules": {
            "llm_inference": {
                "allowed_agents": ["agent-a"],
                "agent_policies": {"agent-a": {"allowed_security_levels": ["PUBLIC"]}}
            },
            "tool_call": {
                "agent_tool_allowlist": {"agent-a": ["test.append"]},
                "high_risk_tools": [], "default_action": "DENY"
            }
        }
    });
    policy
        .ingest_snapshot(&SecurityPolicySnapshot {
            policy_id: "t807-dds".into(),
            version: 1,
            policy_json: document.to_string(),
            published_by: "test".into(),
            timestamp_ns: now_ns(),
        })
        .expect("valid policy");
    policy
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_real_gateways_execute_exactly_100_calls() {
    let temp = TempDir::new("exactly-once-dds");
    let output = temp.path().join("effects.log");
    let claim_store = Arc::new(FileClaimStore::new(&temp.path().join("claims")).expect("claims"));
    let gateway_a_space =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("gateway A DataSpace");
    let gateway_b_space =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).expect("gateway B DataSpace");
    let collector =
        DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).expect("result collector DataSpace");
    let claims_a = Arc::new(ObservedClaims {
        inner: Arc::clone(&claim_store),
        attempts: AtomicUsize::new(0),
        delay: Duration::ZERO,
        loses_every_claim: false,
    });
    let claims_b = Arc::new(ObservedClaims {
        inner: claim_store,
        attempts: AtomicUsize::new(0),
        delay: Duration::ZERO,
        loses_every_claim: true,
    });
    let policy = policy();
    let gateway_a = Arc::new(ToolCallService::with_policy_and_claims(
        gateway_a_space,
        registry(output.clone()),
        Arc::clone(&policy),
        claims_a.clone(),
        OwnerId::parse("gateway-a").expect("owner A"),
    ));
    let gateway_b = Arc::new(ToolCallService::with_policy_and_claims(
        gateway_b_space,
        registry(output.clone()),
        policy,
        claims_b.clone(),
        OwnerId::parse("gateway-b").expect("owner B"),
    ));
    let mut results = collector.subscribe_tool_calls();
    let task_a = tokio::spawn(Arc::clone(&gateway_a).run());
    let task_b = tokio::spawn(Arc::clone(&gateway_b).run());
    tokio::time::sleep(Duration::from_millis(750)).await;

    for index in 0..CALL_COUNT {
        let call = ToolCallRequest {
            call_id: format!("t807-{}-{index}", std::process::id()),
            request_id: format!("request-{index}"),
            requester_id: "agent-a".into(),
            tool_name: "test.append".into(),
            arguments_json: format!("call-{index}"),
            status: status::PENDING,
            created_at_ns: now_ns(),
            ..Default::default()
        };
        gateway_a
            .data_space()
            .write_tool_call(call.clone())
            .await
            .expect("published request");
        gateway_a
            .data_space()
            .write_tool_call(call)
            .await
            .expect("duplicate delivery");
    }

    let terminal_ids = tokio::time::timeout(Duration::from_secs(30), async {
        let mut ids = HashSet::new();
        while ids.len() < CALL_COUNT {
            let Some(result) = mcp_gateway::service::next_tool_call(&mut results).await else {
                break;
            };
            if result.status == status::COMPLETED {
                ids.insert(result.call_id);
            }
        }
        ids
    })
    .await
    .expect("100 terminal DDS results before timeout");

    tokio::time::timeout(Duration::from_secs(10), async {
        while claims_b.attempts.load(Ordering::Relaxed) < CALL_COUNT {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both gateways observed every call");
    task_a.abort();
    task_b.abort();

    let effects = std::fs::read_to_string(&output).expect("effect log");
    let lines: Vec<_> = effects.lines().collect();
    let unique: HashSet<_> = lines.iter().copied().collect();
    assert_eq!(
        terminal_ids.len(),
        CALL_COUNT,
        "terminal results by call_id"
    );
    assert_eq!(lines.len(), CALL_COUNT, "external side-effect count");
    assert_eq!(unique.len(), CALL_COUNT, "duplicate side-effect count");
    assert!(claims_a.attempts.load(Ordering::Relaxed) >= CALL_COUNT);

    eprintln!(
        "terminal_results=100 side_effects=100 unique_effects=100 duplicates=0 gateway_a_claims={} gateway_b_claims={}",
        claims_a.attempts.load(Ordering::Relaxed),
        claims_b.attempts.load(Ordering::Relaxed)
    );
}
