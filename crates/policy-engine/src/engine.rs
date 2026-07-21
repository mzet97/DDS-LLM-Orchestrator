//! Motor local de políticas para chamadas de ferramenta (MCP).
//!
//! Porte fiel de `src/orchestrator/policy_engine/policy_engine.py` (Python):
//! aplica regras de segurança em `ToolCallRequest` **antes** da execução —
//!
//! 1. `security_level` acima de `max_security_level` → `Denied`;
//! 2. whitelist `allowed_tools` não-vazia e ferramenta fora dela → `Denied`;
//! 3. ferramenta na blacklist `denied_tools` → `Denied`;
//! 4. rate limit por agente (janela deslizante de 60 s) → `Denied`;
//! 5. caso contrário → `Allowed`.
//!
//! Sutilezas portadas do Python:
//! - a identidade do agente é `request_id` — o IDL de `ToolCallRequest` não
//!   tem `agent_id`, então `_agent_identity` sempre cai no fallback
//!   (`getattr(request, "agent_id", "") or request.request_id`);
//! - chamadas negadas pelo rate limit não são registradas (só as permitidas
//!   contam na janela).
//!
//! NOTA DE PORTE (desvio decisivo): no Python, `_check_rate_limit` tem um bug
//! latente — o caminho que registra o timestamp (`history.append(now)`) é
//! INALCANÇÁVEL, porque com histórico vazio a função retorna `True` antes de
//! registrar; na prática o rate limit nunca negava nada. Aqui o rate limit é
//! FUNCIONAL, com a seguinte semântica (janela deslizante de 60 s):
//! - a 1ª chamada de uma janela PRIMA a entrada do agente sem contar
//!   (eco do "histórico vazio → permite sem append" do Python);
//! - da 2ª em diante, toda chamada permitida registra o timestamp;
//! - ao atingir `max_calls_per_minute` registros na janela, nega;
//! - janela expirada → entrada removida (prune anti memory-leak) e o ciclo
//!   recomeça (próxima chamada prima de novo, sem contar).

use std::collections::HashSet;

use dashmap::DashMap;
use dds_contract::generated::dds_llm_orchestrator::ToolCallRequest;

use crate::now_ms;

/// Janela deslizante do rate limit por agente, em ms (60 s, como o Python).
pub const RATE_LIMIT_WINDOW_MS: u64 = 60_000;

/// Níveis de segurança — espelha `SecurityLevel` (models.py / IDL).
/// A ordenação segue a criticidade: Public < Internal < Confidential < Restricted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum SecurityLevel {
    Public = 0,
    Internal = 1,
    Confidential = 2,
    Restricted = 3,
}

impl SecurityLevel {
    /// Nome canônico do nível (P1.8: `SecurityLevel(0).name == "PUBLIC"`).
    pub fn name(self) -> &'static str {
        security_level_name(self as i32)
    }
}

/// Nome canônico do nível a partir do inteiro do wire (IDL).
///
/// Valor fora do enum cai em `"PUBLIC"` — mesmo fallback de
/// `llm_gateway._check_policy` (`except (ValueError, TypeError)` no Python).
pub fn security_level_name(level: i32) -> &'static str {
    match level {
        0 => "PUBLIC",
        1 => "INTERNAL",
        2 => "CONFIDENTIAL",
        3 => "RESTRICTED",
        _ => "PUBLIC",
    }
}

/// Status de chamada de ferramenta — espelha `ToolCallStatus` (models.py / IDL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ToolCallStatus {
    Pending = 0,
    Allowed = 1,
    Denied = 2,
    Executing = 3,
    Completed = 4,
    Failed = 5,
}

/// Motor de políticas local (fast path do MCP Gateway).
///
/// Defaults fiéis ao Python: `max_security_level = Confidential`,
/// `max_calls_per_minute = 60`, whitelist/blacklist vazias.
pub struct LocalPolicyEngine {
    max_security_level: SecurityLevel,
    allowed_tools: HashSet<String>,
    denied_tools: HashSet<String>,
    max_calls_per_minute: usize,
    /// Histórico de chamadas por agente (ms desde epoch) — janela deslizante.
    call_history: DashMap<String, Vec<u64>>,
}

impl Default for LocalPolicyEngine {
    /// `PolicyEngine()` do Python: CONFIDENTIAL, sem listas, 60 chamadas/min.
    fn default() -> Self {
        Self::new(
            SecurityLevel::Confidential,
            [] as [&str; 0],
            [] as [&str; 0],
            60,
        )
    }
}

impl LocalPolicyEngine {
    pub fn new(
        max_security_level: SecurityLevel,
        allowed_tools: impl IntoIterator<Item = impl Into<String>>,
        denied_tools: impl IntoIterator<Item = impl Into<String>>,
        max_calls_per_minute: usize,
    ) -> Self {
        Self {
            max_security_level,
            allowed_tools: allowed_tools.into_iter().map(Into::into).collect(),
            denied_tools: denied_tools.into_iter().map(Into::into).collect(),
            max_calls_per_minute,
            call_history: DashMap::new(),
        }
    }

    /// Identidade do agente para o rate limit (porte de `_agent_identity`).
    pub fn agent_identity(request: &ToolCallRequest) -> &str {
        &request.request_id
    }

    /// Avalia se a chamada é permitida (porte de `evaluate`).
    pub fn evaluate(&self, request: &ToolCallRequest) -> ToolCallStatus {
        if request.security_level > self.max_security_level as i32 {
            tracing::warn!(
                call_id = %request.call_id,
                level = security_level_name(request.security_level),
                max = self.max_security_level.name(),
                "ToolCall negada: security_level acima do máximo"
            );
            return ToolCallStatus::Denied;
        }

        if !self.allowed_tools.is_empty() && !self.allowed_tools.contains(&request.tool_name) {
            tracing::warn!(
                call_id = %request.call_id,
                tool = %request.tool_name,
                "ToolCall negada: tool fora da whitelist"
            );
            return ToolCallStatus::Denied;
        }

        if self.denied_tools.contains(&request.tool_name) {
            tracing::warn!(
                call_id = %request.call_id,
                tool = %request.tool_name,
                "ToolCall negada: tool na blacklist"
            );
            return ToolCallStatus::Denied;
        }

        let agent_id = Self::agent_identity(request);
        if !self.check_rate_limit_at(agent_id, now_ms()) {
            tracing::warn!(
                call_id = %request.call_id,
                agent = %agent_id,
                "ToolCall negada: rate limit excedido"
            );
            return ToolCallStatus::Denied;
        }

        tracing::debug!(call_id = %request.call_id, tool = %request.tool_name, "ToolCall permitida");
        ToolCallStatus::Allowed
    }

    /// Rate limit com clock injetado (ms desde epoch) — semântica documentada
    /// na NOTA DE PORTE do módulo: 1ª chamada da janela prima sem contar; as
    /// seguintes registram; `len >= max_calls_per_minute` nega; expiração
    /// remove a entrada (prune anti memory-leak) e reprime o ciclo.
    ///
    /// Separado de `evaluate` para testes determinísticos sem sleep de 60 s.
    #[doc(hidden)]
    pub fn check_rate_limit_at(&self, agent_id: &str, now: u64) -> bool {
        use dashmap::mapref::entry::Entry;

        let window_start = now.saturating_sub(RATE_LIMIT_WINDOW_MS);
        match self.call_history.entry(agent_id.to_string()) {
            // 1ª chamada da janela: prima a entrada (vec vazio = marcador),
            // permite SEM registrar — eco do "histórico vazio → True" do Python.
            Entry::Vacant(slot) => {
                slot.insert(Vec::new());
                true
            }
            Entry::Occupied(mut slot) => {
                let history = slot.get_mut();
                let had_records = !history.is_empty();
                history.retain(|&t| t > window_start);
                if history.is_empty() {
                    if had_records {
                        // Janela expirou: prune e reprime (próxima chamada
                        // conta como 1ª — permite sem registrar).
                        slot.remove();
                    } else {
                        // Entrada primada (1ª chamada já vista): registra.
                        history.push(now);
                    }
                    return true;
                }
                if history.len() >= self.max_calls_per_minute {
                    return false;
                }
                history.push(now);
                true
            }
        }
    }

    /// Número de agentes com histórico ativo (para testes do prune).
    /// Entradas primadas (vec vazio) não contam — são marcadores, não carga.
    #[doc(hidden)]
    pub fn tracked_agents(&self) -> usize {
        self.call_history
            .iter()
            .filter(|e| !e.value().is_empty())
            .count()
    }
}
