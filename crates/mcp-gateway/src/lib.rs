//! # mcp-gateway
//!
//! Port Rust de `src/orchestrator/mcp_gateway/` (~836 LOC Python): o gateway que
//! expõe **ferramentas** ao barramento DDS via tópico `ToolCall.Request`, com
//! governança por política antes da execução.
//!
//! ## Escopo do port (honesto)
//! - **`FilesystemTool` de verdade** (`filesystem.read_file` / `write_file` /
//!   `list_directory`): substitui o servidor MCP oficial
//!   (`npx @modelcontextprotocol/server-filesystem`) que o Python chamava via
//!   stdio. Mesma semântica de sandbox: tudo restrito a uma raiz, com
//!   `canonicalize` + prefix check contra path traversal (inclui symlinks).
//! - **Framework** `ToolHandler` / `ToolRegistry` / despacho por `tool_name`.
//! - **Governança**: trait `PolicyHook` (default `PermissivePolicy`) +
//!   `SecurityPolicy` (fast-path do `PolicyEngine` Python: nível máximo +
//!   allow/deny lists).
//! - **Ferramentas externas** (github/web/database/ci-cd): handlers registrados
//!   que retornam `ToolError::NotConfigured` documentando o que falta — as
//!   chamadas de API (httpx/psycopg/bs4) são follow-up, não foram inventadas.
//!
//! ## Ciclo request→result (paridade com `main.py` Python)
//! 1. Assina `ToolCall.Request`; ignora tudo que não for `status == PENDING`.
//! 2. Política: se negada → `DENIED` + `"Negado pela politica de seguranca local"`.
//! 3. Senão, escreve `EXECUTING` na **mesma instância** (chave = `call_id`).
//! 4. Despacha para o handler: sucesso → `COMPLETED` + `result_json =
//!    {"result": <string>}`; erro → `FAILED` + `error_message`.
//! 5. Escreve o resultado na mesma instância, com `completed_at_ns` estampado.
//!
//! Compile com `--features dds` para o serviço sobre o CycloneDDS real.

pub mod error;
pub mod handler;
pub mod policy;
pub mod service;
pub mod tools;

#[cfg(feature = "dds")]
pub mod dds;

pub use error::ToolError;
pub use handler::{ToolHandler, ToolRegistry};
pub use policy::{PermissivePolicy, PolicyHook, SecurityPolicy};
pub use service::{ServiceError, ToolCallService};
pub use tools::{ExternalTool, FilesystemTool};
