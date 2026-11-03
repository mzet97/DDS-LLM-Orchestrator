//! Avaliação das regras de `policies.json` (distribuídas via `Security.PolicySnapshot`).
//!
//! Porte fiel das duas avaliações existentes no Python:
//!
//! - `llm_gateway/main.py::_check_policy` → [`PolicyDocument::check_llm_request`]
//!   (regra `rules.llm_inference`: `allowed_agents` com sufixo `<agente>-` e
//!   `agent_policies[*].allowed_security_levels`);
//! - `mcp_gateway/main.py::_process` (bloco de cached policy) →
//!   [`PolicyDocument::check_tool_call`] (regra `rules.tool_call`:
//!   `agent_tool_allowlist` e `high_risk_tools`).
//!
//! Defaults quando a regra está ausente (como no Python): documento vazio
//! (`{}`) libera tudo ("ALLOW_ALL"); com a regra presente, agente sem
//! entrada é NEGADO (equivale ao `default_action: "DENY"` do policies.json).
//!
//! Extensão Rust (sem contraparte Python — o Python nunca consumiu
//! `SecurityPolicyUpdate`): [`PolicyDocument::apply_delta`] aplica deltas
//! `ADD_RULE`/`UPDATE_RULE` (deep merge) e `REMOVE_RULE` (remoção de chaves).

use serde_json::Value;

use crate::engine::SecurityLevel;
use crate::error::PolicyError;

/// Resultado da avaliação de uma regra de política.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Permitido por regra explícita ("OK" no Python).
    Allowed,
    /// Permitido por ausência de política configurada ("ALLOW_ALL" no Python).
    AllowedNoPolicy,
    /// Negado — carrega o motivo (mesmo formato das mensagens do Python).
    Denied(String),
}

impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed | Self::AllowedNoPolicy)
    }
}

/// Documento de políticas (conteúdo de `policies.json`).
///
/// Mantido como `serde_json::Value` — como o dict do Python, regras e
/// ausências são resolvidas por acesso com defaults, não por schema rígido.
#[derive(Debug, Clone)]
pub struct PolicyDocument {
    raw: Value,
}

impl PolicyDocument {
    pub fn from_value(raw: Value) -> Self {
        Self { raw }
    }

    pub fn from_json_str(s: &str) -> Result<Self, PolicyError> {
        Ok(Self {
            raw: serde_json::from_str(s)?,
        })
    }

    /// Documento vazio — mesmo efeito de `_cached_policy = {}` (ALLOW_ALL).
    pub fn empty() -> Self {
        Self {
            raw: Value::Object(serde_json::Map::new()),
        }
    }

    /// `policy_data.get("version", 0)` do Python.
    pub fn version(&self) -> i32 {
        self.raw.get("version").and_then(Value::as_i64).unwrap_or(0) as i32
    }

    pub fn as_value(&self) -> &Value {
        &self.raw
    }

    pub fn to_json_string(&self) -> String {
        self.raw.to_string()
    }

    /// `not self._cached_policy` do Python: dict vazio (ou ausente) é falsy.
    pub fn is_empty(&self) -> bool {
        self.raw.as_object().is_none_or(|o| o.is_empty())
    }

    /// Atualiza o campo `version` do documento (usado ao aplicar deltas).
    pub fn set_version(&mut self, version: i32) {
        self.raw["version"] = Value::from(version);
    }

    /// Avalia um pedido de inferência LLM — porte de `_check_policy`.
    ///
    /// (O ramo `provider_constraint == "LOCAL_ONLY"` fica no gateway, não na
    /// política, e não é portado aqui.)
    pub fn check_llm_request(&self, agent_id: &str, security_level: i32) -> PolicyDecision {
        let security_level = match SecurityLevel::try_from(security_level) {
            Ok(level) => level,
            Err(error) => return PolicyDecision::Denied(error.to_string()),
        };
        if self.is_empty() {
            return PolicyDecision::AllowedNoPolicy;
        }

        let llm_rules = self.raw.get("rules").and_then(|r| r.get("llm_inference"));

        let allowed_agents = str_list(llm_rules, "allowed_agents");
        let authorized = allowed_agents
            .iter()
            .any(|a| agent_id == *a || has_agent_suffix(agent_id, a));
        if !authorized {
            return PolicyDecision::Denied(format!(
                "POLICY_DENIED: Agente {agent_id} nao autorizado para inferencia"
            ));
        }

        let agent_policies = llm_rules
            .and_then(|r| r.get("agent_policies"))
            .and_then(Value::as_object);
        let mut agent_policy = agent_policies.and_then(|m| m.get(agent_id));
        // Fallback por prefixo de role: `{}` (ausente ou vazio) é falsy no
        // Python e dispara a busca `agent_id.startswith(role_prefix + "-")`.
        if agent_policy.is_none_or(is_falsy_rule) {
            for prefix in &allowed_agents {
                if has_agent_suffix(agent_id, prefix) {
                    agent_policy = agent_policies.and_then(|m| m.get(*prefix));
                    break;
                }
            }
        }

        let sec_level_str = security_level.name();
        let allowed_levels = str_list(agent_policy, "allowed_security_levels");
        if !allowed_levels.contains(&sec_level_str) {
            return PolicyDecision::Denied(format!(
                "POLICY_DENIED: Agente {agent_id} nao pode usar security_level={sec_level_str}"
            ));
        }

        PolicyDecision::Allowed
    }

    /// Avalia uma chamada de ferramenta — porte do bloco de cached policy do
    /// `mcp_gateway._process` (regra `rules.tool_call`).
    pub fn check_tool_call(&self, agent_id: &str, tool_name: &str) -> PolicyDecision {
        if self.is_empty() {
            return PolicyDecision::AllowedNoPolicy;
        }

        let tool_rules = self.raw.get("rules").and_then(|r| r.get("tool_call"));

        // `agent_allowlist.get(agent_id, [])` — agente sem entrada → DENY.
        let allowed = str_list_direct(
            tool_rules
                .and_then(|t| t.get("agent_tool_allowlist"))
                .and_then(|m| m.get(agent_id)),
        );
        if !allowed.contains(&tool_name) {
            return PolicyDecision::Denied(format!(
                "Tool {tool_name} nao permitida para agente {agent_id}"
            ));
        }

        let high_risk = str_list(tool_rules, "high_risk_tools");
        if high_risk.contains(&tool_name) {
            return PolicyDecision::Denied(format!(
                "Tool {tool_name} e HIGH RISK e sempre negada no MVP"
            ));
        }

        PolicyDecision::Allowed
    }

    /// Aplica um delta de `SecurityPolicyUpdate` ao documento.
    ///
    /// Semântica (nova em Rust — o Python só declarava as operações):
    /// - `ADD_RULE` / `UPDATE_RULE`: deep merge do delta no documento
    ///   (objetos fundem recursivamente; arrays/escalares são substituídos);
    /// - `REMOVE_RULE`: remove as chaves folha presentes no delta
    ///   (objetos aninhados navegam; qualquer outro valor remove a chave).
    ///
    /// O delta raiz precisa ser um objeto; caso contrário, `InvalidDelta`.
    pub fn apply_delta(&mut self, operation: &str, delta: &Value) -> Result<(), PolicyError> {
        if !delta.is_object() {
            return Err(PolicyError::InvalidDelta(
                "rule_delta_json precisa ser um objeto JSON na raiz".into(),
            ));
        }
        match operation {
            "ADD_RULE" | "UPDATE_RULE" => {
                deep_merge(&mut self.raw, delta);
                Ok(())
            }
            "REMOVE_RULE" => {
                remove_paths(&mut self.raw, delta);
                Ok(())
            }
            op => Err(PolicyError::InvalidDelta(format!(
                "operação desconhecida: {op} (esperado ADD_RULE, REMOVE_RULE ou UPDATE_RULE)"
            ))),
        }
    }
}

/// `agent_id.startswith(prefix + "-")` do Python (instâncias de um role).
fn has_agent_suffix(agent_id: &str, prefix: &str) -> bool {
    agent_id
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('-'))
}

/// Lista de strings em `value[key]` (`dict.get(key, [])` do Python).
fn str_list<'v>(value: Option<&'v Value>, key: &str) -> Vec<&'v str> {
    str_list_direct(value.and_then(|v| v.get(key)))
}

/// Lista de strings diretamente em `value` (quando o próprio valor é o array).
fn str_list_direct(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// `{}` ou `null` é falsy no Python (`if not agent_policy:`).
fn is_falsy_rule(v: &Value) -> bool {
    v.is_null() || v.as_object().is_some_and(|o| o.is_empty())
}

/// Deep merge: objetos fundem recursivamente; qualquer outro tipo substitui.
fn deep_merge(base: &mut Value, delta: &Value) {
    match (&mut *base, delta) {
        (Value::Object(b), Value::Object(d)) => {
            for (k, v) in d {
                match b.get_mut(k) {
                    Some(bv) => deep_merge(bv, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (base, delta) => *base = delta.clone(),
    }
}

/// Remove as chaves folha indicadas pelo delta (objetos aninhados navegam).
fn remove_paths(base: &mut Value, delta: &Value) {
    if let (Value::Object(b), Value::Object(d)) = (&mut *base, delta) {
        for (k, v) in d {
            if v.is_object() {
                if let Some(bv) = b.get_mut(k) {
                    remove_paths(bv, v);
                }
            } else {
                b.remove(k);
            }
        }
    }
}
