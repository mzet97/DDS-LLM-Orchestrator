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

    /// Falha na camada DataSpace (publicação/assinatura).
    #[error("dataspace: {0}")]
    DataSpace(#[from] dds_dataspace::api::DataSpaceError),
}
