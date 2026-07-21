//! # policy-engine
//!
//! Motor de políticas de segurança do orquestrador DDS-LLM — porte de
//! `src/orchestrator/policy_engine/` (Python) para Rust.
//!
//! Componentes (cada um porte fiel da contraparte Python):
//!
//! - [`engine`]: avaliação local de `ToolCallRequest` (fast path do MCP
//!   Gateway) — `policy_engine.py`: nível de segurança máximo, whitelist/
//!   blacklist de ferramentas e rate limit por agente (janela de 60 s).
//! - [`rules`]: avaliação das regras de `policies.json` — `_check_policy`
//!   do `llm_gateway` (regra `llm_inference`) e o bloco de cached policy do
//!   `mcp_gateway` (regra `tool_call`), incluindo os defaults quando a regra
//!   está ausente.
//! - [`cache`]: cache local de políticas com TTL (análogo ao `_NullRedis`
//!   do Python: sempre disponível, sem dependência externa — dashmap em
//!   memória substitui o Redis).
//! - [`service`]: serviço fonte da verdade (`main.py`): lê `policies.json`,
//!   publica `Security.PolicySnapshot` na mudança de versão, re-publica a
//!   cada 60 s e aplica deltas de `Security.PolicyUpdate`.
//!
//! A distribuição usa os tópicos/tipos já prontos do contrato DDS
//! (`dds_contract::topics::SECURITY_POLICY_SNAPSHOT` / `..._UPDATE`).
//! Compile com `--features dds` para o runtime CycloneDDS real.

pub mod cache;
pub mod engine;
pub mod error;
pub mod rules;
pub mod service;

pub use cache::PolicyCache;
pub use engine::{security_level_name, LocalPolicyEngine, SecurityLevel, ToolCallStatus};
pub use error::PolicyError;
pub use rules::{PolicyDecision, PolicyDocument};
pub use service::PolicyEngineService;

/// Milissegundos desde o UNIX epoch (mesmo relógio de `time.time()` do Python,
/// usado na janela do rate limit e na expiração do cache).
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Nanossegundos desde o UNIX epoch (`now_ns()` do models.py — timestamps do wire).
pub(crate) fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
