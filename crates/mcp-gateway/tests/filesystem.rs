//! Testes do `FilesystemTool`: read/write/list OK dentro da raiz; path
//! traversal (`..`, absoluto, symlink) negado; limites de tamanho; erro JSON
//! padronizado.

mod common;

use common::TempDir;
use mcp_gateway::tools::{FilesystemTool, FsLimits};
use mcp_gateway::{ToolError, ToolRegistry};

/// Registry com os 3 handlers filesystem.* sobre a raiz temporária.
fn registry_with_fs(root: &std::path::Path) -> ToolRegistry {
    let registry = ToolRegistry::new();
    for tool in FilesystemTool::ops(root).expect("ops") {
        registry.register(tool);
    }
    registry
}

fn args(v: serde_json::Value) -> String {
    v.to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_write_list_ok() {
    let tmp = TempDir::new("fs-ok");
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    let registry = registry_with_fs(tmp.path());

    // write_file cria arquivo novo dentro da raiz
    let out = registry
        .dispatch(
            FilesystemTool::WRITE_FILE,
            &args(serde_json::json!({"path": "sub/nota.txt", "content": "conteudo da nota"})),
        )
        .await
        .expect("write");
    assert_eq!(out, "Successfully wrote to sub/nota.txt");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("sub/nota.txt")).unwrap(),
        "conteudo da nota"
    );

    // read_file devolve o conteúdo
    let out = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": "sub/nota.txt"})),
        )
        .await
        .expect("read");
    assert_eq!(out, "conteudo da nota");

    // list_directory marca [DIR]/[FILE] ordenado (formato do servidor MCP oficial)
    let out = registry
        .dispatch(
            FilesystemTool::LIST_DIRECTORY,
            &args(serde_json::json!({"path": "."})),
        )
        .await
        .expect("list");
    assert_eq!(out, "[DIR] sub");
    let out = registry
        .dispatch(
            FilesystemTool::LIST_DIRECTORY,
            &args(serde_json::json!({"path": "sub"})),
        )
        .await
        .expect("list sub");
    assert_eq!(out, "[FILE] nota.txt");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn traversal_dotdot_negado() {
    let tmp = TempDir::new("fs-dotdot");
    // Arquivo FORA da raiz (irmão do sandbox), alvo do escape.
    let outside = tmp
        .path()
        .parent()
        .unwrap()
        .join(format!("segredo-{}", std::process::id()));
    std::fs::write(&outside, "segredo fora da raiz").unwrap();
    let registry = registry_with_fs(tmp.path());

    let err = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": format!("../{}", outside.file_name().unwrap().to_string_lossy())})),
        )
        .await
        .expect_err(".. deve ser negado");
    assert!(
        matches!(err, ToolError::PathTraversal(_)),
        "esperava PathTraversal, veio {err:?}"
    );

    // JSON de erro padronizado: {"error":{"code": "...", "message": "..."}}
    let json: serde_json::Value = serde_json::from_str(&err.to_error_json()).unwrap();
    assert_eq!(json["error"]["code"], "PATH_TRAVERSAL");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("path traversal"));

    let _ = std::fs::remove_file(&outside);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn traversal_absoluto_negado() {
    let tmp = TempDir::new("fs-abs");
    let registry = registry_with_fs(tmp.path());

    let err = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": "/etc/passwd"})),
        )
        .await
        .expect_err("path absoluto fora da raiz deve ser negado");
    assert!(matches!(err, ToolError::PathTraversal(_)), "veio {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn traversal_symlink_negado() {
    let tmp = TempDir::new("fs-symlink");
    let outside = tmp
        .path()
        .parent()
        .unwrap()
        .join(format!("alvo-{}", std::process::id()));
    std::fs::write(&outside, "fora via symlink").unwrap();
    std::os::unix::fs::symlink(&outside, tmp.path().join("link")).unwrap();
    let registry = registry_with_fs(tmp.path());

    // Leitura através de symlink que aponta para fora: negada.
    let err = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": "link"})),
        )
        .await
        .expect_err("symlink para fora deve ser negado");
    assert!(matches!(err, ToolError::PathTraversal(_)), "veio {err:?}");

    // Escrita através do mesmo symlink: também negada.
    let err = registry
        .dispatch(
            FilesystemTool::WRITE_FILE,
            &args(serde_json::json!({"path": "link", "content": "x"})),
        )
        .await
        .expect_err("escrita via symlink para fora deve ser negada");
    assert!(matches!(err, ToolError::PathTraversal(_)), "veio {err:?}");

    let _ = std::fs::remove_file(&outside);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn symlink_component_inside_root_is_denied() {
    let tmp = TempDir::new("fs-internal-symlink");
    std::fs::create_dir(tmp.path().join("real")).unwrap();
    std::fs::write(tmp.path().join("real/secret.txt"), "inside").unwrap();
    std::os::unix::fs::symlink("real", tmp.path().join("link")).unwrap();
    let registry = registry_with_fs(tmp.path());

    let err = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": "link/secret.txt"})),
        )
        .await
        .expect_err("every symlink component must be denied");
    assert!(matches!(err, ToolError::PathTraversal(_)), "veio {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn write_traversal_dotdot_negado() {
    let tmp = TempDir::new("fs-wdotdot");
    let registry = registry_with_fs(tmp.path());

    let err = registry
        .dispatch(
            FilesystemTool::WRITE_FILE,
            &args(serde_json::json!({"path": "../evil.txt", "content": "x"})),
        )
        .await
        .expect_err("escrita fora da raiz deve ser negada");
    assert!(matches!(err, ToolError::PathTraversal(_)), "veio {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn limites_de_tamanho() {
    let tmp = TempDir::new("fs-limits");
    std::fs::write(tmp.path().join("grande.txt"), "123456789").unwrap();
    let registry = ToolRegistry::new();
    let limits = FsLimits {
        max_read_bytes: 4,
        max_write_bytes: 4,
        ..Default::default()
    };
    for tool in FilesystemTool::ops_with_limits(tmp.path(), limits).expect("ops") {
        registry.register(tool);
    }

    // Leitura acima do limite.
    let err = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": "grande.txt"})),
        )
        .await
        .expect_err("leitura acima do limite");
    assert!(
        matches!(err, ToolError::TooLarge { size: 9, max: 4 }),
        "veio {err:?}"
    );
    assert_eq!(err.code(), "TOO_LARGE");

    // Escrita acima do limite.
    let err = registry
        .dispatch(
            FilesystemTool::WRITE_FILE,
            &args(serde_json::json!({"path": "novo.txt", "content": "12345"})),
        )
        .await
        .expect_err("escrita acima do limite");
    assert!(
        matches!(err, ToolError::TooLarge { size: 5, max: 4 }),
        "veio {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nao_encontrado_e_argumentos_invalidos() {
    let tmp = TempDir::new("fs-misc");
    let registry = registry_with_fs(tmp.path());

    // Arquivo inexistente dentro da raiz.
    let err = registry
        .dispatch(
            FilesystemTool::READ_FILE,
            &args(serde_json::json!({"path": "nao-existe.txt"})),
        )
        .await
        .expect_err("inexistente");
    assert!(matches!(err, ToolError::NotFound(_)), "veio {err:?}");

    // JSON malformado.
    let err = registry
        .dispatch(FilesystemTool::READ_FILE, "{isso nao e json")
        .await
        .expect_err("json quebrado");
    assert!(
        matches!(err, ToolError::InvalidArguments(_)),
        "veio {err:?}"
    );

    // Campo obrigatório ausente.
    let err = registry
        .dispatch(
            FilesystemTool::WRITE_FILE,
            &args(serde_json::json!({"path": "x.txt"})),
        )
        .await
        .expect_err("sem content");
    assert!(
        matches!(err, ToolError::InvalidArguments(_)),
        "veio {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn internal_claim_directory_is_not_a_tool_path() {
    let tmp = TempDir::new("fs-claims-private");
    std::fs::create_dir(tmp.path().join(".mcp-claims")).unwrap();
    std::fs::write(tmp.path().join(".mcp-claims/claim"), "owner").unwrap();
    let registry = registry_with_fs(tmp.path());
    for path in [".mcp-claims/claim", "./.mcp-claims/claim"] {
        let error = registry
            .dispatch(
                FilesystemTool::READ_FILE,
                &args(serde_json::json!({"path": path})),
            )
            .await
            .expect_err("claim storage stays private");
        assert!(matches!(error, ToolError::PathTraversal(_)));
    }
}
