//! Stubs documentados das ferramentas externas (github/web/database/ci-cd).
//!
//! Escopo honesto do port: os clientes Python (`mcp_github_client.py`,
//! `mcp_web_client.py`, `mcp_database_client.py`, `mcp_cicd_client.py`) chamam
//! APIs externas (GitHub REST, DuckDuckGo, PostgreSQL, GitHub Actions) via
//! httpx/psycopg/bs4. Essas chamadas NÃO foram inventadas aqui — cada tool
//! name vira um handler registrado que retorna `ToolError::NotConfigured`
//! explicando o que falta (credencial/cliente), até o follow-up do port.

use crate::error::ToolError;
use crate::handler::{ToolFuture, ToolHandler};

/// Handler stub de uma ferramenta externa ainda não portada.
///
/// Mantém o tool name REGISTRADO no barramento (contrato visível), mas toda
/// chamada falha com `NotConfigured` documentando o que falta.
pub struct ExternalTool {
    name: &'static str,
    backend: &'static str,
    missing: &'static str,
}

impl ExternalTool {
    /// Cria um stub: `name` é o tool name canônico, `backend` o serviço
    /// externo, `missing` o que falta para o port (credencial/cliente).
    pub const fn new(name: &'static str, backend: &'static str, missing: &'static str) -> Self {
        Self {
            name,
            backend,
            missing,
        }
    }
}

impl ToolHandler for ExternalTool {
    fn name(&self) -> &str {
        self.name
    }

    fn handle<'a>(&'a self, _arguments_json: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            Err(ToolError::NotConfigured(format!(
                "'{}' (backend {}): {} — chamadas de API externas são follow-up do port Rust",
                self.name, self.backend, self.missing
            )))
        })
    }
}

/// Stubs das 14 ferramentas externas do `mcp_gateway` Python.
///
/// Nomes idênticos aos registrados pelos clientes Python
/// (`github.*`, `web.*`, `database.*`, `cicd.*`).
pub fn external_tools() -> Vec<ExternalTool> {
    use ExternalTool as E;
    vec![
        // mcp_github_client.py (GitHub REST API, GITHUB_TOKEN)
        E::new(
            "github.search_code",
            "GitHub REST API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        E::new(
            "github.get_file",
            "GitHub REST API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        E::new(
            "github.list_issues",
            "GitHub REST API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        E::new(
            "github.get_repo_info",
            "GitHub REST API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        // mcp_web_client.py (DuckDuckGo HTML / fetch de URL)
        E::new(
            "web.search",
            "DuckDuckGo HTML",
            "cliente HTTP + parsing HTML não portados",
        ),
        E::new(
            "web.fetch",
            "fetch de URL",
            "cliente HTTP + extração de texto não portados",
        ),
        // mcp_database_client.py (PostgreSQL via psycopg, DATABASE_URL)
        E::new(
            "database.query",
            "PostgreSQL",
            "requer DATABASE_URL e driver postgres",
        ),
        E::new(
            "database.execute",
            "PostgreSQL",
            "requer DATABASE_URL e driver postgres",
        ),
        E::new(
            "database.list_tables",
            "PostgreSQL",
            "requer DATABASE_URL e driver postgres",
        ),
        E::new(
            "database.describe_table",
            "PostgreSQL",
            "requer DATABASE_URL e driver postgres",
        ),
        // mcp_cicd_client.py (GitHub Actions, GITHUB_TOKEN)
        E::new(
            "cicd.list_workflows",
            "GitHub Actions API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        E::new(
            "cicd.trigger_workflow",
            "GitHub Actions API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        E::new(
            "cicd.get_workflow_status",
            "GitHub Actions API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
        E::new(
            "cicd.list_artifacts",
            "GitHub Actions API",
            "requer GITHUB_TOKEN e cliente HTTP",
        ),
    ]
}
