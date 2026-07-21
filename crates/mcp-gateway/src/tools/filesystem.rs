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
//! Segurança: todo path é resolvido contra a raiz com `canonicalize` + prefix
//! check — `..`, paths absolutos e **symlinks** que escapem da raiz viram
//! `ToolError::PathTraversal`. Leitura/escrita têm limites de tamanho
//! (`FsLimits`).

use crate::error::ToolError;
use crate::handler::{ToolFuture, ToolHandler};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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
    root: PathBuf,
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
        let root = canonical_root(root.as_ref())?;
        Ok([FsOp::ReadFile, FsOp::WriteFile, FsOp::ListDirectory]
            .into_iter()
            .map(|op| Self {
                root: root.clone(),
                limits,
                op,
            })
            .collect())
    }

    /// Raiz canonicalizada do sandbox.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve `raw` contra a raiz e garante que o resultado fica DENTRO dela.
    ///
    /// - `must_exist = true` (read/list): canonicaliza o path inteiro (resolve
    ///   `..` e symlinks) — inexistente vira `NotFound`.
    /// - `must_exist = false` (write): se já existir (arquivo ou symlink),
    ///   canonicaliza tudo; senão canonicaliza o pai (que precisa existir) e
    ///   anexa o nome do arquivo. Um symlink apontando para fora é pego pelo
    ///   prefix check nos dois casos.
    fn resolve(&self, raw: &str, must_exist: bool) -> Result<PathBuf, ToolError> {
        if raw.is_empty() {
            return Err(ToolError::InvalidArguments("path vazio".into()));
        }
        // join() com path absoluto SUBSTITUI a raiz — o prefix check abaixo
        // é o que nega o escape ("/etc/passwd" nunca começa com a raiz).
        let candidate = self.root.join(raw);

        if must_exist || candidate.symlink_metadata().is_ok() {
            let canon = candidate.canonicalize().map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ToolError::NotFound(raw.to_string())
                } else {
                    ToolError::Io(e)
                }
            })?;
            return self.check_within(canon, raw);
        }

        // Arquivo novo: o pai precisa existir e estar dentro da raiz.
        let parent = candidate
            .parent()
            .ok_or_else(|| ToolError::InvalidArguments(format!("path inválido: '{raw}'")))?;
        let canon_parent = parent.canonicalize().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound(format!("diretório pai de '{raw}'"))
            } else {
                ToolError::Io(e)
            }
        })?;
        let canon_parent = self.check_within(canon_parent, raw)?;
        let file_name = candidate.file_name().ok_or_else(|| {
            ToolError::InvalidArguments(format!("path sem nome de arquivo: '{raw}'"))
        })?;
        Ok(canon_parent.join(file_name))
    }

    /// Prefix check: `canon` (já canonicalizado) precisa estar sob a raiz.
    fn check_within(&self, canon: PathBuf, raw: &str) -> Result<PathBuf, ToolError> {
        if canon.starts_with(&self.root) {
            Ok(canon)
        } else {
            Err(ToolError::PathTraversal(raw.to_string()))
        }
    }

    async fn read_file(&self, arguments_json: &str) -> Result<String, ToolError> {
        let args: PathArgs = parse_args(arguments_json)?;
        let path = self.resolve(&args.path, true)?;
        let meta = tokio::fs::metadata(&path).await?;
        if meta.is_dir() {
            return Err(ToolError::InvalidArguments(format!(
                "'{}' é um diretório (use {})",
                args.path,
                Self::LIST_DIRECTORY
            )));
        }
        if meta.len() > self.limits.max_read_bytes {
            return Err(ToolError::TooLarge {
                size: meta.len(),
                max: self.limits.max_read_bytes,
            });
        }
        let bytes = tokio::fs::read(&path).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
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
        let path = self.resolve(&args.path, false)?;
        tokio::fs::write(&path, &args.content).await?;
        // Mensagem idêntica à do servidor MCP oficial de filesystem.
        Ok(format!("Successfully wrote to {}", args.path))
    }

    async fn list_directory(&self, arguments_json: &str) -> Result<String, ToolError> {
        let args: PathArgs = parse_args(arguments_json)?;
        let path = self.resolve(&args.path, true)?;
        let meta = tokio::fs::metadata(&path).await?;
        if !meta.is_dir() {
            return Err(ToolError::InvalidArguments(format!(
                "'{}' não é um diretório",
                args.path
            )));
        }
        let mut entries = Vec::new();
        let mut rd = tokio::fs::read_dir(&path).await?;
        while let Some(entry) = rd.next_entry().await? {
            let file_type = entry.file_type().await?;
            // Formato do servidor MCP oficial: "[FILE] nome" / "[DIR] nome".
            let tag = if file_type.is_dir() {
                "[DIR]"
            } else {
                "[FILE]"
            };
            entries.push(format!("{tag} {}", entry.file_name().to_string_lossy()));
        }
        entries.sort();
        if entries.len() > self.limits.max_list_entries {
            let omitted = entries.len() - self.limits.max_list_entries;
            entries.truncate(self.limits.max_list_entries);
            entries.push(format!("... ({omitted} entradas omitidas)"));
        }
        Ok(entries.join("\n"))
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

/// Cria a raiz (se ausente) e a canonicaliza.
fn canonical_root(root: &Path) -> Result<PathBuf, ToolError> {
    if !root.exists() {
        std::fs::create_dir_all(root)?;
    }
    Ok(root.canonicalize()?)
}

/// Parse de `arguments_json` com erro padronizado.
fn parse_args<'a, T: Deserialize<'a>>(arguments_json: &'a str) -> Result<T, ToolError> {
    serde_json::from_str(arguments_json)
        .map_err(|e| ToolError::InvalidArguments(format!("json inválido: {e}")))
}
