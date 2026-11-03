//! Erros da crate `policy-engine` (thiserror, convenção das libs do workspace).

/// Erros do motor/serviço de políticas.
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    /// `policy_json` / `rule_delta_json` inválido (parse serde_json).
    #[error("JSON de política inválido: {0}")]
    Json(#[from] serde_json::Error),

    /// Delta de `SecurityPolicyUpdate` malformado ou operação desconhecida.
    #[error("delta de regra inválido: {0}")]
    InvalidDelta(String),

    /// Update/republish com versão que regride em relação ao estado atual
    /// (REQ/T-820-16: um update atrasado ou o re-publish do arquivo não
    /// podem reverter versão+documento — paridade com o `StaleVersion` do
    /// `mcp-gateway`).
    #[error("versão de política defasada (regressão rejeitada)")]
    StaleVersion,

    /// Falha na camada DataSpace (publicação/assinatura).
    #[error("dataspace: {0}")]
    DataSpace(#[from] dds_dataspace::api::DataSpaceError),
}
