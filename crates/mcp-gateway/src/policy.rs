//! Governança: trait `PolicyHook` — o ponto de extensão de política aplicado
//! a todo `ToolCall.Request` antes da execução (o `policy.evaluate` do Python).

use std::collections::HashSet;

/// Níveis de segurança (espelha `SecurityLevel` do `models.py` Python / IDL).
pub mod security_level {
    /// Público (0).
    pub const PUBLIC: i32 = 0;
    /// Interno (1).
    pub const INTERNAL: i32 = 1;
    /// Confidencial (2) — teto default da `SecurityPolicy`, como no Python.
    pub const CONFIDENTIAL: i32 = 2;
    /// Restrito (3).
    pub const RESTRICTED: i32 = 3;
}

/// Hook de governança: decide se uma chamada de ferramenta pode executar.
///
/// Retorna `true` para permitir, `false` para negar (o serviço grava
/// `status = DENIED` na instância, como o Python).
pub trait PolicyHook: Send + Sync {
    /// Avalia `(tool_name, security_level, arguments_json)` de um request.
    fn check(&self, tool_name: &str, security_level: i32, arguments_json: &str) -> bool;
}

/// Política **default permissiva**: permite tudo (equivalente ao gateway
/// Python rodando sem regras carregadas).
#[derive(Debug, Default, Clone, Copy)]
pub struct PermissivePolicy;

impl PolicyHook for PermissivePolicy {
    fn check(&self, _tool_name: &str, _security_level: i32, _arguments_json: &str) -> bool {
        true
    }
}

/// Fast-path do `PolicyEngine` Python: nível máximo de segurança +
/// allow/deny lists de ferramentas.
///
/// Desvio consciente: o rate-limit por agente (60 calls/min) do Python não
/// entra aqui — a assinatura do hook não recebe `agent_id`.
pub struct SecurityPolicy {
    /// Nível máximo aceito (default `CONFIDENTIAL` = 2, como o Python).
    pub max_security_level: i32,
    /// Whitelist: se não-vazia, só tools listadas passam.
    pub allowed_tools: HashSet<String>,
    /// Blacklist: tools listadas são sempre negadas.
    pub denied_tools: HashSet<String>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            max_security_level: security_level::CONFIDENTIAL,
            allowed_tools: HashSet::new(),
            denied_tools: HashSet::new(),
        }
    }
}

impl PolicyHook for SecurityPolicy {
    fn check(&self, tool_name: &str, security_level: i32, _arguments_json: &str) -> bool {
        if security_level > self.max_security_level {
            return false;
        }
        if !self.allowed_tools.is_empty() && !self.allowed_tools.contains(tool_name) {
            return false;
        }
        if self.denied_tools.contains(tool_name) {
            return false;
        }
        true
    }
}
