//! Diagnóstico da falha de claim (temporário): distingue onde as 100
//! execuções se perdem — entrega, reivindicação, handler ou observador.
//! Só roda em domínio isolado (95) e store em memória.

#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::{api::DataSpaceApi, DataSpace};
use mcp_gateway::handler::ToolHandler;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::service::{next_tool_call, status};
use mcp_gateway::{MemoryClaimStore, OwnerId, ToolCallService, ToolRegistry};
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 95;
const TOTAL: usize = 100;

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

/// Decorator de ClaimStore: registra quais call_ids cada owner ganhou.
struct CountingStore {
    inner: MemoryClaimStore,
    owner_gw1: OwnerId,
    owner_gw2: OwnerId,
    won_gw1: Mutex<HashSet<String>>,
    won_gw2: Mutex<HashSet<String>>,
    already: AtomicUsize,
}

impl CountingStore {
    fn new(owner_gw1: OwnerId, owner_gw2: OwnerId) -> Self {
        Self {
            inner: MemoryClaimStore::default(),
            owner_gw1,
            owner_gw2,
            won_gw1: Mutex::new(HashSet::new()),
            won_gw2: Mutex::new(HashSet::new()),
            already: AtomicUsize::new(0),
        }
    }
}

impl mcp_gateway::ClaimStore for CountingStore {
    fn try_claim(
        &self,
        call_id: &str,
        owner: &OwnerId,
    ) -> Result<mcp_gateway::ClaimDecision, mcp_gateway::ClaimError> {
        let decision = self.inner.try_claim(call_id, owner)?;
        match decision {
            mcp_gateway::ClaimDecision::Won => {
                let is_gw1 = *owner == self.owner_gw1;
                let is_gw2 = *owner == self.owner_gw2;
                if is_gw1 {
                    self.won_gw1.lock().unwrap().insert(call_id.to_owned());
                } else if is_gw2 {
                    self.won_gw2.lock().unwrap().insert(call_id.to_owned());
                }
            }
            mcp_gateway::ClaimDecision::AlreadyClaimed => {
                self.already.fetch_add(1, Ordering::SeqCst);
            }
        }
        Ok(decision)
    }
}

/// Handler que registra os ids executados (a partir do arguments_json).
#[derive(Clone)]
struct RecordingTool {
    count: Arc<AtomicUsize>,
    executed: Arc<Mutex<HashSet<String>>>,
}

impl ToolHandler for RecordingTool {
    fn name(&self) -> &str {
        "echo.tool"
    }

    fn handle<'a>(
        &'a self,
        arguments_json: &'a str,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<String, mcp_gateway::ToolError>> + Send + 'a>,
    > {
        self.count.fetch_add(1, Ordering::SeqCst);
        let id: String = arguments_json
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        self.executed.lock().unwrap().insert(id);
        let text = arguments_json.to_string();
        Box::pin(async move { Ok(text) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn diag_claim_dumps_sets_on_timeout() {
    let count = Arc::new(AtomicUsize::new(0));
    let executed = Arc::new(Mutex::new(HashSet::new()));

    let data_space_one = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).expect("ds-1");
    let data_space_two = DataSpace::new(DOMAIN, DataSpace::STRENGTH_AGENT).expect("ds-2");
    let observer = DataSpace::new(DOMAIN, DataSpace::STRENGTH_CLIENT).expect("observer");

    let registry_one = {
        let registry = ToolRegistry::new();
        registry.register_arc(Arc::new(RecordingTool {
            count: Arc::clone(&count),
            executed: Arc::clone(&executed),
        }));
        registry
    };
    let registry_two = {
        let registry = ToolRegistry::new();
        registry.register_arc(Arc::new(RecordingTool {
            count: Arc::clone(&count),
            executed: Arc::clone(&executed),
        }));
        registry
    };

    let counting = Arc::new(CountingStore::new(
        OwnerId::parse("claim-gw-1").expect("owner 1"),
        OwnerId::parse("claim-gw-2").expect("owner 2"),
    ));
    let policy = allowed_policy();
    let service_one = Arc::new(ToolCallService::with_policy_and_claims(
        data_space_one,
        registry_one,
        Arc::clone(&policy),
        Arc::clone(&counting) as Arc<dyn mcp_gateway::ClaimStore>,
        OwnerId::parse("claim-gw-1").expect("owner 1"),
    ));
    let service_two = Arc::new(ToolCallService::with_policy_and_claims(
        data_space_two,
        registry_two,
        policy,
        Arc::clone(&counting) as Arc<dyn mcp_gateway::ClaimStore>,
        OwnerId::parse("claim-gw-2").expect("owner 2"),
    ));

    let seen = Arc::new(tokio::sync::Mutex::new(HashSet::new()));
    let mut stream = Box::pin(observer.subscribe_tool_calls());
    let collector = {
        let seen = Arc::clone(&seen);
        tokio::spawn(async move {
            while seen.lock().await.len() < TOTAL {
                let Some(call) = next_tool_call(&mut stream).await else {
                    break;
                };
                if call.status == status::COMPLETED {
                    assert_eq!(call.error_message, "");
                    seen.lock().await
                        .insert(call.call_id.trim_start_matches("tool-call-").to_owned());
                }
            }
        })
    };
    let run_one = tokio::spawn({
        let service_one = Arc::clone(&service_one);
        async move { service_one.run().await.expect("gateway-1") }
    });
    let run_two = tokio::spawn({
        let service_two = Arc::clone(&service_two);
        async move { service_two.run().await.expect("gateway-2") }
    });

    tokio::time::sleep(Duration::from_millis(750)).await;

    let written_set: HashSet<String> =
        (0..TOTAL).map(|i| i.to_string()).collect();
    for i in 0..TOTAL {
        let call = make_tool_call(i);
        service_one
            .data_space()
            .write_tool_call(call.clone())
            .await
            .expect("escreve tool call");
        service_one
            .data_space()
            .write_tool_call(call)
            .await
            .expect("duplicate delivery");
    }

    let timed_out = tokio::time::timeout(Duration::from_secs(30), async {
        while seen.lock().await.len() < TOTAL {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .is_err();

    collector.abort();
    run_one.abort();
    run_two.abort();

    // Dump diagnóstico
    let seen_guard = seen.lock().await.clone();
    let won1 = counting.won_gw1.lock().unwrap().clone();
    let won2 = counting.won_gw2.lock().unwrap().clone();
    let claimed_all: HashSet<String> = won1.union(&won2).cloned().collect();
    let executed_guard = executed.lock().unwrap().clone();
    println!("DIAG timeout={timed_out}");
    println!("DIAG publicados únicos: {}", written_set.len());
    println!(
        "DIAG claims ganhos gw1={} gw2={} total_único={} já_reivindicados={}",
        won1.len(),
        won2.len(),
        claimed_all.len(),
        counting.already.load(Ordering::SeqCst)
    );
    let nunca: Vec<_> = written_set.difference(&claimed_all).collect();
    println!("DIAG nunca reivindicados ({}): {:?}", nunca.len(), nunca);
    println!("DIAG handler executou: {} únicos ({} chamadas)", executed_guard.len(), count.load(Ordering::SeqCst));
    let reclamados_sem_exec: Vec<_> = claimed_all.difference(&executed_guard).collect();
    println!(
        "DIAG reivindicados mas NÃO executados ({}): {:?}",
        reclamados_sem_exec.len(),
        reclamados_sem_exec
    );
    println!("DIAG observador viu concluídos: {}", seen_guard.len());
    let executados_sem_obs: Vec<_> = executed_guard.difference(&seen_guard).collect();
    println!(
        "DIAG executados mas NÃO observados ({}): {:?}",
        executados_sem_obs.len(),
        executados_sem_obs
    );
    let inesperados: Vec<_> = seen_guard.difference(&written_set).collect();
    println!(
        "DIAG observados sem ter sido publicados ({}): {:?}",
        inesperados.len(),
        inesperados
    );

    assert!(!timed_out, "DIAG: timeout com os conjuntos acima");
}
