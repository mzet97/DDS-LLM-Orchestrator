//! Validação isolada do executor (G-18+ parcial): chamada real fim a fim.
//!
//! Diretório próprio (`temp`), só arquivos de teste, sem malha DDS real
//! (`InMemoryDataSpace`), sem shell genérico: o executor só enxerga a
//! raiz confinada. Prova: PENDING → COMPLETED com resultado correlacionado
//! na MESMA instância; negação com mensagem; e PENDING intacto quando não
//! há executor — nunca retorno simulado.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dds_contract::generated::dds_llm_orchestrator::{SecurityPolicySnapshot, ToolCallRequest};
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::in_memory::InMemoryDataSpace;
use mcp_gateway::handler::ToolRegistry;
use mcp_gateway::policy::DistributedPolicy;
use mcp_gateway::service::{status, ToolCallService};
use mcp_gateway::tools::FilesystemTool;

const POLICIES_JSON: &str = include_str!("../../policy-engine/policies.json");

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mcp-exec-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("raiz de teste");
    dir
}

fn pending_call(tool: &str, requester: &str, arguments_json: &str) -> ToolCallRequest {
    ToolCallRequest {
        call_id: format!("call-{tool}-{requester}"),
        request_id: String::from("req-1"),
        requester_id: String::from(requester),
        tool_name: String::from(tool),
        arguments_json: String::from(arguments_json),
        security_level: 0,
        status: status::PENDING,
        result_json: String::new(),
        error_message: String::new(),
        created_at_ns: now_ns(),
        completed_at_ns: 0,
    }
}

fn service_with_snapshot(root: &std::path::Path) -> ToolCallService<InMemoryDataSpace> {
    let space = InMemoryDataSpace::new();
    let registry = ToolRegistry::new();
    for tool in FilesystemTool::ops(root).expect("ops fs") {
        registry.register(tool);
    }
    let policy = Arc::new(DistributedPolicy::default());
    policy
        .ingest_snapshot(&SecurityPolicySnapshot {
            policy_id: String::from("default"),
            version: 2,
            policy_json: String::from(POLICIES_JSON),
            published_by: String::from("teste-isolado"),
            timestamp_ns: now_ns(),
        })
        .expect("snapshot válido");
    ToolCallService::with_policy(space, registry, policy)
}

#[tokio::test]
async fn real_call_completes_on_same_instance() {
    let root = temp_root("ok");
    std::fs::write(root.join("prova.txt"), "conteudo-real").expect("prova");
    let service = service_with_snapshot(&root);
    let space = service.data_space();
    let request = pending_call(
        FilesystemTool::READ_FILE,
        "TestAgent",
        r#"{"path":"prova.txt"}"#,
    );
    space
        .write_tool_call(request.clone())
        .await
        .expect("publica");

    let done = service.process_one(&request).await.expect("processa");

    assert_eq!(done.status, status::COMPLETED);
    assert_eq!(done.call_id, request.call_id);
    assert!(done.result_json.contains("conteudo-real"));
    assert!(done.completed_at_ns >= done.created_at_ns);
    let stored = space
        .read_tool_call(&request.call_id)
        .await
        .expect("le")
        .expect("instancia existe");
    assert_eq!(stored.status, status::COMPLETED);
    assert_eq!(stored.result_json, done.result_json);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn unauthorized_tool_is_denied_with_message() {
    let root = temp_root("negado");
    let service = service_with_snapshot(&root);
    let space = service.data_space();
    let request = pending_call(
        FilesystemTool::LIST_DIRECTORY,
        "TestAgent",
        r#"{"path":"."}"#,
    );
    space
        .write_tool_call(request.clone())
        .await
        .expect("publica");

    let done = service.process_one(&request).await.expect("processa");

    assert_eq!(done.status, status::DENIED);
    assert!(!done.error_message.is_empty());
    let stored = space
        .read_tool_call(&request.call_id)
        .await
        .expect("le")
        .expect("instancia existe");
    assert_eq!(stored.status, status::DENIED);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn path_outside_root_fails_without_escape() {
    let root = temp_root("fuga");
    let service = service_with_snapshot(&root);
    let request = pending_call(
        FilesystemTool::READ_FILE,
        "TestAgent",
        r#"{"path":"../fora.txt"}"#,
    );

    let done = service.process_one(&request).await.expect("processa");

    assert_eq!(done.status, status::FAILED);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn pending_stays_pending_without_executor() {
    let space = InMemoryDataSpace::new();
    let request = pending_call(
        FilesystemTool::READ_FILE,
        "TestAgent",
        r#"{"path":"prova.txt"}"#,
    );
    space
        .write_tool_call(request.clone())
        .await
        .expect("publica");

    let stored = space
        .read_tool_call(&request.call_id)
        .await
        .expect("le")
        .expect("instancia existe");

    assert_eq!(stored.status, status::PENDING);
    assert!(stored.result_json.is_empty());
}
