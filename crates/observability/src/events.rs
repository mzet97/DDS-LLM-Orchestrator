//! Schema de eventos de observabilidade unificados.
//!
//! Porte de `src/orchestrator/observability/events.py`. Os nomes dos tipos de
//! evento no JSON são **idênticos** aos do Python (`EventType.name`, incluindo
//! o caso misto `QoS_VIOLATION`) para manter o JSONL retrocompatível com os
//! arquivos já gravados pela malha Python.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tipos de evento de observabilidade (paridade com `EventType` do Python —
/// mesmos valores inteiros do `IntEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum EventType {
    RequestReceived = 0,
    AgentSelected = 1,
    ContextUpdated = 2,
    AgentResponseReceived = 3,
    ResultConsolidated = 4,
    LlmRequestSent = 10,
    LlmResultReceived = 11,
    LlmErrorReceived = 12,
    ToolCallRequested = 20,
    ToolCallExecuted = 21,
    ToolCallDenied = 22,
    PolicyDenied = 30,
    PolicyApplied = 31,
    QosViolation = 40,
    Error = 50,
}

impl EventType {
    /// Nome canônico igual ao `EventType.name` do Python (vai para o JSONL).
    pub fn name(self) -> &'static str {
        match self {
            Self::RequestReceived => "REQUEST_RECEIVED",
            Self::AgentSelected => "AGENT_SELECTED",
            Self::ContextUpdated => "CONTEXT_UPDATED",
            Self::AgentResponseReceived => "AGENT_RESPONSE_RECEIVED",
            Self::ResultConsolidated => "RESULT_CONSOLIDATED",
            Self::LlmRequestSent => "LLM_REQUEST_SENT",
            Self::LlmResultReceived => "LLM_RESULT_RECEIVED",
            Self::LlmErrorReceived => "LLM_ERROR_RECEIVED",
            Self::ToolCallRequested => "TOOL_CALL_REQUESTED",
            Self::ToolCallExecuted => "TOOL_CALL_EXECUTED",
            Self::ToolCallDenied => "TOOL_CALL_DENIED",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::PolicyApplied => "POLICY_APPLIED",
            Self::QosViolation => "QoS_VIOLATION",
            Self::Error => "ERROR",
        }
    }

    /// Resolve pelo nome canônico (inverso de [`Self::name`]).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "REQUEST_RECEIVED" => Self::RequestReceived,
            "AGENT_SELECTED" => Self::AgentSelected,
            "CONTEXT_UPDATED" => Self::ContextUpdated,
            "AGENT_RESPONSE_RECEIVED" => Self::AgentResponseReceived,
            "RESULT_CONSOLIDATED" => Self::ResultConsolidated,
            "LLM_REQUEST_SENT" => Self::LlmRequestSent,
            "LLM_RESULT_RECEIVED" => Self::LlmResultReceived,
            "LLM_ERROR_RECEIVED" => Self::LlmErrorReceived,
            "TOOL_CALL_REQUESTED" => Self::ToolCallRequested,
            "TOOL_CALL_EXECUTED" => Self::ToolCallExecuted,
            "TOOL_CALL_DENIED" => Self::ToolCallDenied,
            "POLICY_DENIED" => Self::PolicyDenied,
            "POLICY_APPLIED" => Self::PolicyApplied,
            "QoS_VIOLATION" => Self::QosViolation,
            "ERROR" => Self::Error,
            _ => return None,
        })
    }

    /// Valor inteiro do `IntEnum` Python.
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// Default igual ao `query()` do `file_sink.py` (`EventType[data.get(..., "ERROR")]`).
impl Default for EventType {
    fn default() -> Self {
        Self::Error
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for EventType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::from_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("event_type desconhecido: {name}")))
    }
}

/// Timestamp atual em nanossegundos (wall clock), como `time.time_ns()`.
#[must_use]
pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Gera id de evento de 12 chars hex — substitui `uuid.uuid4().hex[:12]` sem
/// adicionar a dependência `uuid`: xor-fold de `now_ns()` com um contador
/// atômico (unicidade em processo pelo contador; entre processos pelo tempo).
fn new_event_id() -> String {
    let ctr = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = (now_ns() ^ ctr.wrapping_mul(0x9E37_79B9_7F4A_7C15)) & 0xFFFF_FFFF_FFFF;
    format!("{mixed:012x}")
}

/// Evento de observabilidade (paridade com `ObservabilityEvent` do Python; a
/// ordem dos campos casa com `to_dict()` para gerar JSONL no mesmo formato).
///
/// No `Deserialize`, campos ausentes caem nos defaults — mesmo comportamento
/// do `query()` do `file_sink.py` (`data.get(campo, "")` / `0` / `{}` / ERROR).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityEvent {
    pub event_id: String,
    pub event_type: EventType,
    pub task_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub component_id: String,
    pub message: String,
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub timestamp_ns: u64,
}

impl ObservabilityEvent {
    /// Cria evento com id novo e `timestamp_ns = now` (defaults do dataclass).
    pub fn new(event_type: EventType) -> Self {
        Self {
            event_id: new_event_id(),
            event_type,
            timestamp_ns: now_ns(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_names_match_python() {
        // Paridade 1:1 com o IntEnum de events.py (nome <-> valor).
        let cases = [
            (EventType::RequestReceived, "REQUEST_RECEIVED", 0),
            (EventType::AgentSelected, "AGENT_SELECTED", 1),
            (EventType::ContextUpdated, "CONTEXT_UPDATED", 2),
            (
                EventType::AgentResponseReceived,
                "AGENT_RESPONSE_RECEIVED",
                3,
            ),
            (EventType::ResultConsolidated, "RESULT_CONSOLIDATED", 4),
            (EventType::LlmRequestSent, "LLM_REQUEST_SENT", 10),
            (EventType::LlmResultReceived, "LLM_RESULT_RECEIVED", 11),
            (EventType::LlmErrorReceived, "LLM_ERROR_RECEIVED", 12),
            (EventType::ToolCallRequested, "TOOL_CALL_REQUESTED", 20),
            (EventType::ToolCallExecuted, "TOOL_CALL_EXECUTED", 21),
            (EventType::ToolCallDenied, "TOOL_CALL_DENIED", 22),
            (EventType::PolicyDenied, "POLICY_DENIED", 30),
            (EventType::PolicyApplied, "POLICY_APPLIED", 31),
            (EventType::QosViolation, "QoS_VIOLATION", 40),
            (EventType::Error, "ERROR", 50),
        ];
        for (ty, name, code) in cases {
            assert_eq!(ty.name(), name);
            assert_eq!(ty.code(), code);
            assert_eq!(EventType::from_name(name), Some(ty));
        }
        assert_eq!(EventType::from_name("NOPE"), None);
    }

    #[test]
    fn event_id_is_12_hex_chars() {
        let ev = ObservabilityEvent::new(EventType::RequestReceived);
        assert_eq!(ev.event_id.len(), 12);
        assert!(ev.event_id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(ev.timestamp_ns > 0);
    }

    #[test]
    fn event_ids_are_unique_in_process() {
        let a = ObservabilityEvent::new(EventType::Error);
        let b = ObservabilityEvent::new(EventType::Error);
        assert_ne!(a.event_id, b.event_id);
    }

    #[test]
    fn json_roundtrip_matches_python_to_dict_keys() {
        let mut ev = ObservabilityEvent::new(EventType::LlmResultReceived);
        ev.task_id = "task-1".into();
        ev.metadata.insert("k".into(), serde_json::Value::from(1));
        let json = serde_json::to_string(&ev).expect("serialize");
        // Mesmas chaves e ordem do to_dict() do Python.
        assert_eq!(
            json,
            format!(
                "{{\"event_id\":\"{}\",\"event_type\":\"LLM_RESULT_RECEIVED\",\
                 \"task_id\":\"task-1\",\"request_id\":\"\",\"agent_id\":\"\",\
                 \"component_id\":\"\",\"message\":\"\",\"metadata\":{{\"k\":1}},\
                 \"timestamp_ns\":{}}}",
                ev.event_id, ev.timestamp_ns
            )
            .replace(' ', "")
        );
        let back: ObservabilityEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.event_type, EventType::LlmResultReceived);
        assert_eq!(back.task_id, "task-1");
    }

    #[test]
    fn deserialize_with_missing_fields_uses_python_query_defaults() {
        let ev: ObservabilityEvent = serde_json::from_str("{}").expect("defaults");
        assert_eq!(ev.event_type, EventType::Error);
        assert_eq!(ev.task_id, "");
        assert_eq!(ev.timestamp_ns, 0);
        assert!(ev.metadata.is_empty());
    }
}
