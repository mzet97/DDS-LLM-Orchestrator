//! `FilesystemTool` — filesystem restrito a uma raiz (sandbox).
//!
//! Substitui o servidor MCP oficial `@modelcontextprotocol/server-filesystem`
//! que o `mcp_client.py` Python consumia via stdio. Operações (mesmos nomes,
//! com o prefixo `filesystem.` que o cliente DDS envia):
//!
//! | tool                      | args                        | resultado                     |
//! |---------------------------|-----------------------------|-------------------------------|
//! | `filesystem.read_file`    | `{path}`                    | conteúdo do arquivo (string)  |
//! | `filesystem.write_file`   | `{path, content}`           | confirmação                   |
//! | `filesystem.list_directory` | `{path}`                  | linhas `[FILE] x` / `[DIR] d` |
//!
//! Segurança: operações usam um directory fd fixo e `openat2` com
//! `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS` até o open final. Leitura/escrita
//! têm limites de tamanho (`FsLimits`).

use crate::error::ToolError;
use crate::handler::{ToolFuture, ToolHandler};
use crate::tools::sandbox::SandboxRoot;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

/// Limites de tamanho do sandbox.
#[derive(Debug, Clone, Copy)]
pub struct FsLimits {
    /// Máximo de bytes lidos por `read_file` (default 1 MiB).
    pub max_read_bytes: u64,
    /// Máximo de bytes escritos por `write_file` (default 1 MiB).
    pub max_write_bytes: u64,
    /// Máximo de entradas devolvidas por `list_directory` (default 1000).
    pub max_list_entries: usize,
}

impl Default for FsLimits {
    fn default() -> Self {
        Self {
            max_read_bytes: 1024 * 1024,
            max_write_bytes: 1024 * 1024,
            max_list_entries: 1000,
        }
    }
}

/// Operação coberta por uma instância de `FilesystemTool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FsOp {
    ReadFile,
    WriteFile,
    ListDirectory,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
}

/// Handler das operações `filesystem.*` — cada instância cobre UMA operação,
/// todas compartilhando a mesma raiz canonicalizada e limites.
pub struct FilesystemTool {
    root: Arc<SandboxRoot>,
    limits: FsLimits,
    op: FsOp,
}

impl FilesystemTool {
    /// Tool name da operação de leitura.
    pub const READ_FILE: &'static str = "filesystem.read_file";
    /// Tool name da operação de escrita.
    pub const WRITE_FILE: &'static str = "filesystem.write_file";
    /// Tool name da operação de listagem.
    pub const LIST_DIRECTORY: &'static str = "filesystem.list_directory";

    /// Cria os 3 handlers (read/write/list) sobre `root` com limites default.
    ///
    /// A raiz é criada se não existir e canonicalizada (resolve `..`/symlinks
    /// do próprio caminho da raiz).
    pub fn ops(root: impl AsRef<Path>) -> Result<Vec<Self>, ToolError> {
        Self::ops_with_limits(root, FsLimits::default())
    }

    /// Igual a [`Self::ops`], com limites customizados.
    pub fn ops_with_limits(
        root: impl AsRef<Path>,
        limits: FsLimits,
    ) -> Result<Vec<Self>, ToolError> {
        let root = Arc::new(SandboxRoot::open(root.as_ref())?);
        Ok([FsOp::ReadFile, FsOp::WriteFile, FsOp::ListDirectory]
            .into_iter()
            .map(|op| Self {
                root: Arc::clone(&root),
                limits,
                op,
            })
            .collect())
    }

    /// Raiz canonicalizada do sandbox.
    pub fn root(&self) -> &Path {
        self.root.display_path()
    }

    async fn read_file(&self, arguments_json: &str) -> Result<String, ToolError> {
        let args: PathArgs = parse_args(arguments_json)?;
        self.root.read(&args.path, self.limits.max_read_bytes)
    }

    async fn write_file(&self, arguments_json: &str) -> Result<String, ToolError> {
        let args: WriteArgs = parse_args(arguments_json)?;
        let size = args.content.len() as u64;
        if size > self.limits.max_write_bytes {
            return Err(ToolError::TooLarge {
                size,
                max: self.limits.max_write_bytes,
            });
        }
        self.root.write(&args.path, args.content.as_bytes())?;
        // Mensagem idêntica à do servidor MCP oficial de filesystem.
        Ok(format!("Successfully wrote to {}", args.path))
    }

    async fn list_directory(&self, arguments_json: &str) -> Result<String, ToolError> {
        let args: PathArgs = parse_args(arguments_json)?;
        self.root.list(&args.path, self.limits.max_list_entries)
    }
}

impl ToolHandler for FilesystemTool {
    fn name(&self) -> &str {
        match self.op {
            FsOp::ReadFile => Self::READ_FILE,
            FsOp::WriteFile => Self::WRITE_FILE,
            FsOp::ListDirectory => Self::LIST_DIRECTORY,
        }
    }

    fn handle<'a>(&'a self, arguments_json: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            match self.op {
                FsOp::ReadFile => self.read_file(arguments_json).await,
                FsOp::WriteFile => self.write_file(arguments_json).await,
                FsOp::ListDirectory => self.list_directory(arguments_json).await,
            }
        })
    }
}

/// Parse de `arguments_json` com erro padronizado.
fn parse_args<'a, T: Deserialize<'a>>(arguments_json: &'a str) -> Result<T, ToolError> {
    serde_json::from_str(arguments_json)
        .map_err(|e| ToolError::InvalidArguments(format!("json inválido: {e}")))
}
