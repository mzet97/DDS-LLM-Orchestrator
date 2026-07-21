//! Testes do `ToolRegistry`: registro, despacho por tool_name, UnknownTool e
//! stubs externos documentados (`NotConfigured`).

use mcp_gateway::handler::ToolFuture;
use mcp_gateway::tools::external_tools;
use mcp_gateway::{ToolError, ToolHandler, ToolRegistry};

/// Handler de teste: devolve os argumentos com prefixo.
struct EchoTool;

impl ToolHandler for EchoTool {
    fn name(&self) -> &str {
        "test.echo"
    }

    fn handle<'a>(&'a self, arguments_json: &'a str) -> ToolFuture<'a> {
        Box::pin(async move { Ok(format!("echo:{arguments_json}")) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registro_e_despacho() {
    let registry = ToolRegistry::new();
    registry.register(EchoTool);

    assert!(registry.contains("test.echo"));
    assert_eq!(registry.len(), 1);

    let out = registry
        .dispatch("test.echo", r#"{"a":1}"#)
        .await
        .expect("dispatch");
    assert_eq!(out, r#"echo:{"a":1}"#);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ferramenta_desconhecida() {
    let registry = ToolRegistry::new();
    let err = registry
        .dispatch("nao.existe", "{}")
        .await
        .expect_err("tool sem handler");
    assert!(matches!(err, ToolError::UnknownTool(_)), "veio {err:?}");
    assert_eq!(err.code(), "UNKNOWN_TOOL");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_tools_ordenado() {
    let registry = ToolRegistry::new();
    registry.register(EchoTool);
    for stub in external_tools() {
        registry.register(stub);
    }

    let tools = registry.list_tools();
    assert_eq!(tools.len(), 15); // 14 stubs externos + test.echo
    let mut sorted = tools.clone();
    sorted.sort();
    assert_eq!(tools, sorted, "list_tools deve vir ordenado");
    assert!(tools.contains(&"github.search_code".to_string()));
    assert!(tools.contains(&"web.fetch".to_string()));
    assert!(tools.contains(&"database.query".to_string()));
    assert!(tools.contains(&"cicd.trigger_workflow".to_string()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stubs_externos_retornam_not_configured_documentado() {
    let registry = ToolRegistry::new();
    for stub in external_tools() {
        registry.register(stub);
    }

    // GitHub: mensagem documenta a credencial que falta.
    let err = registry
        .dispatch("github.search_code", r#"{"query":"dds"}"#)
        .await
        .expect_err("github sem credencial");
    assert!(matches!(err, ToolError::NotConfigured(_)), "veio {err:?}");
    assert!(err.to_string().contains("GITHUB_TOKEN"), "msg: {err}");
    assert_eq!(err.code(), "NOT_CONFIGURED");

    // Web / database / cicd: mesmo contrato.
    for (tool, trecho) in [
        ("web.search", "DuckDuckGo"),
        ("database.execute", "DATABASE_URL"),
        ("cicd.list_workflows", "GITHUB_TOKEN"),
    ] {
        let err = registry
            .dispatch(tool, "{}")
            .await
            .expect_err("stub deve falhar com NotConfigured");
        assert!(
            matches!(err, ToolError::NotConfigured(_)),
            "{tool}: {err:?}"
        );
        assert!(err.to_string().contains(trecho), "{tool}: {err}");
    }
}
