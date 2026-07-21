//! Erros das ferramentas MCP, com JSON de erro padronizado.
//!
//! Toda falha de ferramenta vira `ToolError`: o serviço grava `to_string()` em
//! `error_message` (paridade com o `str(e)` do Python) e `to_error_json()` fica
//! disponível para consumidores que queiram o formato estruturado.

/// Erro de execução de uma ferramenta.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// `arguments_json` malformado ou com campos obrigatórios ausentes.
    #[error("argumentos inválidos: {0}")]
    InvalidArguments(String),

    /// Path resolvido foge da raiz do sandbox (`..`, absoluto ou symlink).
    #[error("path traversal negado: '{0}' foge da raiz do sandbox")]
    PathTraversal(String),

    /// Arquivo/diretório não encontrado dentro da raiz.
    #[error("não encontrado: {0}")]
    NotFound(String),

    /// Leitura/escrita acima do limite configurado.
    #[error("tamanho {size} bytes excede o limite de {max} bytes")]
    TooLarge {
        /// Tamanho efetivo.
        size: u64,
        /// Limite configurado.
        max: u64,
    },

    /// `tool_name` sem handler registrado no registry.
    #[error("ferramenta desconhecida: {0}")]
    UnknownTool(String),

    /// Ferramenta externa sem credencial/implementação (follow-up do port).
    #[error("ferramenta não configurada: {0}")]
    NotConfigured(String),

    /// Erro de E/S do filesystem.
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),
}

impl ToolError {
    /// Código estável do erro (para o JSON padronizado e logs estruturados).
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArguments(_) => "INVALID_ARGUMENTS",
            Self::PathTraversal(_) => "PATH_TRAVERSAL",
            Self::NotFound(_) => "NOT_FOUND",
            Self::TooLarge { .. } => "TOO_LARGE",
            Self::UnknownTool(_) => "UNKNOWN_TOOL",
            Self::NotConfigured(_) => "NOT_CONFIGURED",
            Self::Io(_) => "IO_ERROR",
        }
    }

    /// JSON de erro padronizado: `{"error":{"code": "...", "message": "..."}}`.
    pub fn to_error_json(&self) -> String {
        serde_json::json!({
            "error": {
                "code": self.code(),
                "message": self.to_string(),
            }
        })
        .to_string()
    }
}
