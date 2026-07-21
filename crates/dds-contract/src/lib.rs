//! # dds-contract
//!
//! O **contrato DDS único**: tipos de tópico (gerados do IDL) e perfis de QoS.
//! Substitui a manutenção manual de `dds_backend/dds_types.py` — os tipos vêm do
//! **mesmo IDL** que o C++ (`OrchestratorDDS.idl`) e do V4 (`OrchestratorV4.idl`).
//!
//! ## Requisitos
//! - REQ-001: geração via `build.rs` + `cyclonedds-build` (não editar gerado)
//! - REQ-002/003: typename + @key / LLM keyless
//! - REQ-004: perfis online/estrutural
//! - REQ-005: round-trip XCDR (`--features dds`)
//! - REQ-006/007: roles + nomes canônicos
//!
//! Compile com `--features dds` para gerar e usar os tipos reais.

/// Nomes canônicos dos tópicos (iguais aos do Python/C++). REQ-007.
pub mod topics {
    pub const TASKS: &str = "Tasks";
    pub const AGENT_REGISTRY: &str = "AgentRegistry";
    pub const TASK_OUTPUT: &str = "TaskOutput";
    pub const SYSTEM_METRICS: &str = "SystemMetrics";
    pub const LLM_REQUEST: &str = "LLM.InferenceRequest";
    pub const LLM_RESULT: &str = "LLM.InferenceResult";
    pub const LLM_ERROR: &str = "LLM.InferenceError";
    pub const SERVER_STATUS: &str = "ServerStatus";
    pub const QOS_ROUTING_PROFILE: &str = "QoS.RoutingProfile";
    pub const CONTEXT_SNAPSHOT: &str = "Context.Snapshot";
    pub const CONTEXT_UPDATE: &str = "Context.Update";
    pub const TOOL_CALL_REQUEST: &str = "ToolCall.Request";
    pub const EXECUTION_TRACE: &str = "Execution.Trace";
    pub const SECURITY_POLICY_SNAPSHOT: &str = "Security.PolicySnapshot";
    pub const SECURITY_POLICY_UPDATE: &str = "Security.PolicyUpdate";
    pub const QOS_METRIC: &str = "QoS.Metric";
    pub const QOS_VIOLATION: &str = "QoS.Violation";
    pub const QOS_DISCOVERY: &str = "QoS.Discovery";
}

/// Perfis de QoS (nomes iguais ao decisor NFCM/fuzzy). REQ-007.
pub mod profiles {
    pub const ALL: [&str; 5] = [
        "QoS_Critical",
        "QoS_Failover",
        "QoS_StreamLike",
        "QoS_LowCost",
        "QoS_Balanced",
    ];
}

pub mod qos;
pub mod roles;

pub use qos::{
    all_profiles, qos_profile, DurabilityKind, HistoryKind, LivelinessKind, OnlineKnobs,
    OwnershipKind, ReliabilityKind, StructuralQos, UnknownProfile,
};
pub use roles::{STRENGTH_AGENT, STRENGTH_CLIENT, STRENGTH_ORCHESTRATOR};

/// Tipos gerados do IDL (só com feature `dds`).
#[cfg(feature = "dds")]
pub mod generated {
    #![allow(unused_imports, dead_code, non_camel_case_types, non_snake_case)]

    /// Tipos de `OrchestratorDDS.idl` (module `orchestrator`).
    ///
    /// `empty_line_after_outer_attr` é suprimido porque o `.rs` incluído é
    /// gerado pelo `cyclonedds-idlc` (third_party/) a cada build — não é
    /// código nosso para reformatar.
    pub mod llm {
        #![allow(unused_imports, dead_code, non_camel_case_types, non_snake_case)]
        #![allow(clippy::empty_line_after_outer_attr)]
        include!(concat!(env!("OUT_DIR"), "/OrchestratorDDS.rs"));
    }

    /// Tipos de `OrchestratorV4.idl` (module `dds_llm_orchestrator`). Ver nota
    /// sobre código gerado no módulo `llm` acima.
    pub mod v4 {
        #![allow(unused_imports, dead_code, non_camel_case_types, non_snake_case)]
        #![allow(clippy::empty_line_after_outer_attr)]
        include!(concat!(env!("OUT_DIR"), "/OrchestratorV4.rs"));
    }

    // Reexports convenientes (paths estáveis para o resto da migração).
    pub use llm::orchestrator;
    pub use v4::dds_llm_orchestrator;
}

/// Mock types for compilation without DDS feature (for testing/development).
#[cfg(not(feature = "dds"))]
pub mod generated {
    pub mod orchestrator {
        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct LLMInferenceRequest {
            pub request_id: String,
            pub task_id: String,
            pub agent_id: String,
            pub model_name: String,
            pub messages_json: String,
            pub temperature: f32,
            pub max_tokens: u32,
            pub stream: bool,
            pub security_level: i32,
            pub provider_constraint: String,
            pub created_at_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct LLMInferenceResult {
            pub request_id: String,
            pub seq_num: u32,
            pub content: String,
            pub is_final: bool,
            pub finish_reason: i32,
            pub model_used: String,
            pub tokens_prompt: u32,
            pub tokens_completion: u32,
            pub emitted_at_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct LLMInferenceError {
            pub request_id: String,
            pub error_code: i32,
            pub error_message: String,
            pub provider: String,
            pub retriable: bool,
            pub emitted_at_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct ServerStatus {
            pub server_id: String,
            pub slots_idle: i32,
            pub slots_processing: i32,
            pub model_loaded: String,
            pub ready: bool,
        }
    }

    pub mod dds_llm_orchestrator {
        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct Task {
            pub task_id: String,
            pub client_id: String,
            pub assigned_agent: String,
            pub target_agent: String,
            pub model_required: i32,
            pub model_name: String,
            pub messages_json: String,
            pub temperature: f32,
            pub max_tokens: u32,
            pub stream: bool,
            pub status: i32,
            pub priority: i32,
            pub created_at_ns: u64,
            pub assigned_at_ns: u64,
            pub started_at_ns: u64,
            pub completed_at_ns: u64,
            pub deadline_ns: u64,
            pub retry_count: u32,
            pub finish_reason: String,
            pub t_serialization_ns: u64,
            pub t_transport_send_ns: u64,
            pub t_agent_queue_ns: u64,
            pub t_inference_ns: u64,
            pub t_transport_return_ns: u64,
            pub t_deserialization_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct AgentState {
            pub agent_id: String,
            pub hostname: String,
            pub model: String,
            pub specialization: String,
            pub slots_total: u32,
            pub slots_busy: u32,
            pub vram_total_mb: u32,
            pub vram_used_mb: u32,
            pub ema_latency_ms: f32,
            pub completed_total: u32,
            pub failed_total: u32,
            pub health: i32,
            pub last_update_ns: u64,
            pub uptime_seconds: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct TaskOutput {
            pub task_id: String,
            pub seq_num: u32,
            pub content: String,
            pub is_final: bool,
            pub finish_reason: i32,
            pub agent_id: String,
            pub token_count: u32,
            pub emitted_at_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct SystemMetric {
            pub metric_name: String,
            pub component_id: String,
            pub component_type: i32,
            pub value: f32,
            pub unit: String,
            pub timestamp_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct QoSRoutingProfile {
            pub profile_id: String,
            pub version: i32,
            pub profile_name: String,
            pub preferred_agent_prefix: String,
            pub allowed_agent_prefixes_json: String,
            pub weights_json: String,
            pub fallback_after_ms: i32,
            pub centroid_score: f32,
            pub explanation_json: String,
            pub timestamp_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct ContextSnapshot {
            pub context_id: String,
            pub client_id: String,
            pub session_id: String,
            pub messages_json: String,
            pub metadata_json: String,
            pub security_level: i32,
            pub created_at_ns: u64,
            pub updated_at_ns: u64,
            pub ttl_seconds: u32,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct ContextUpdate {
            pub context_id: String,
            pub update_type: i32,
            pub messages_delta_json: String,
            pub metadata_delta_json: String,
            pub updated_at_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct ToolCallRequest {
            pub call_id: String,
            pub request_id: String,
            pub tool_name: String,
            pub arguments_json: String,
            pub security_level: i32,
            pub status: i32,
            pub result_json: String,
            pub error_message: String,
            pub created_at_ns: u64,
            pub completed_at_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct ExecutionTraceEvent {
            pub trace_id: String,
            pub seq_num: u32,
            pub event_type: i32,
            pub task_id: String,
            pub request_id: String,
            pub agent_id: String,
            pub component_id: String,
            pub component_type: i32,
            pub payload_json: String,
            pub timestamp_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct SecurityPolicySnapshot {
            pub policy_id: String,
            pub version: i32,
            pub policy_json: String,
            pub published_by: String,
            pub timestamp_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct SecurityPolicyUpdate {
            pub policy_id: String,
            pub previous_version: i32,
            pub new_version: i32,
            pub operation: String,
            pub rule_delta_json: String,
            pub published_by: String,
            pub timestamp_ns: u64,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct QoSMetric {
            pub metric_id: String,
            pub metric_name: String,
            pub component: String,
            pub value: i64,
            pub delta: i32,
            pub window_ms: i32,
            pub timestamp_ns: u64,
            pub experiment_id: String,
            pub run_id: String,
            pub qos_profile: String,
            pub load_level: String,
            pub network_condition: String,
            pub payload_level: String,
            pub replication_idx: i32,
            pub warmup: bool,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct QoSViolation {
            pub violation_id: String,
            pub violation_type: String,
            pub topic_name: String,
            pub component: String,
            pub entity_kind: String,
            pub affected_entity: String,
            pub severity: String,
            pub details_json: String,
            pub timestamp_ns: u64,
            pub experiment_id: String,
            pub run_id: String,
            pub qos_profile: String,
            pub load_level: String,
            pub network_condition: String,
            pub payload_level: String,
            pub replication_idx: i32,
            pub warmup: bool,
        }

        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        pub struct DiscoveryEvent {
            pub event_id: String,
            pub event_type: String,
            pub topic_name: String,
            pub local_entity: String,
            pub remote_entity: String,
            pub count_change: i32,
            pub timestamp_ns: u64,
        }
    }
}

/// Reexporta o runtime CycloneDDS.
#[cfg(feature = "dds")]
pub mod dds {
    pub use cyclonedds as rt;
}

/// Typenames canônicos (módulo IDL + struct), alinhados a C++ `m_typename` e
/// Python `typename=`. REQ-002.
pub mod typenames {
    pub const LLM_INFERENCE_REQUEST: &str = "orchestrator::LLMInferenceRequest";
    pub const LLM_INFERENCE_RESULT: &str = "orchestrator::LLMInferenceResult";
    pub const LLM_INFERENCE_ERROR: &str = "orchestrator::LLMInferenceError";
    pub const SERVER_STATUS: &str = "orchestrator::ServerStatus";
    pub const TASK: &str = "dds_llm_orchestrator::Task";
    pub const AGENT_STATE: &str = "dds_llm_orchestrator::AgentState";
    pub const TASK_OUTPUT: &str = "dds_llm_orchestrator::TaskOutput";
    pub const SYSTEM_METRIC: &str = "dds_llm_orchestrator::SystemMetric";
    pub const QOS_ROUTING_PROFILE: &str = "dds_llm_orchestrator::QoSRoutingProfile";
    pub const CONTEXT_SNAPSHOT: &str = "dds_llm_orchestrator::ContextSnapshot";
    pub const CONTEXT_UPDATE: &str = "dds_llm_orchestrator::ContextUpdate";
    pub const TOOL_CALL_REQUEST: &str = "dds_llm_orchestrator::ToolCallRequest";
    pub const EXECUTION_TRACE_EVENT: &str = "dds_llm_orchestrator::ExecutionTraceEvent";
    pub const SECURITY_POLICY_SNAPSHOT: &str = "dds_llm_orchestrator::SecurityPolicySnapshot";
    pub const SECURITY_POLICY_UPDATE: &str = "dds_llm_orchestrator::SecurityPolicyUpdate";
    pub const QOS_METRIC: &str = "dds_llm_orchestrator::QoSMetric";
    pub const QOS_VIOLATION: &str = "dds_llm_orchestrator::QoSViolation";
    pub const DISCOVERY_EVENT: &str = "dds_llm_orchestrator::DiscoveryEvent";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_match_context() {
        assert_eq!(topics::TASKS, "Tasks");
        assert_eq!(topics::AGENT_REGISTRY, "AgentRegistry");
        assert_eq!(topics::TASK_OUTPUT, "TaskOutput");
        assert_eq!(topics::LLM_REQUEST, "LLM.InferenceRequest");
        assert_eq!(topics::LLM_RESULT, "LLM.InferenceResult");
        assert_eq!(topics::LLM_ERROR, "LLM.InferenceError");
    }

    #[test]
    fn profiles_match_nfcm() {
        assert_eq!(profiles::ALL.len(), 5);
        assert!(profiles::ALL.contains(&"QoS_Critical"));
        assert!(profiles::ALL.contains(&"QoS_StreamLike"));
        assert!(profiles::ALL.contains(&"QoS_Balanced"));
        assert!(profiles::ALL.contains(&"QoS_Failover"));
        assert!(profiles::ALL.contains(&"QoS_LowCost"));
    }

    #[test]
    fn typenames_are_module_qualified() {
        assert!(typenames::LLM_INFERENCE_REQUEST.starts_with("orchestrator::"));
        assert!(typenames::TASK.starts_with("dds_llm_orchestrator::"));
        assert!(!typenames::LLM_INFERENCE_REQUEST.contains(' '));
    }
}

#[cfg(all(test, feature = "dds"))]
mod dds_tests {
    use super::*;
    use cyclonedds::{CdrDeserializer, CdrEncoding, CdrSerializer, DdsType};
    use generated::dds_llm_orchestrator::{AgentState, SystemMetric, Task, TaskOutput};
    use generated::orchestrator::{
        LLMInferenceError, LLMInferenceRequest, LLMInferenceResult, ServerStatus,
    };

    #[test]
    fn wire_typenames_match_idl_modules() {
        assert_eq!(
            LLMInferenceRequest::type_name(),
            typenames::LLM_INFERENCE_REQUEST
        );
        assert_eq!(
            LLMInferenceResult::type_name(),
            typenames::LLM_INFERENCE_RESULT
        );
        assert_eq!(
            LLMInferenceError::type_name(),
            typenames::LLM_INFERENCE_ERROR
        );
        assert_eq!(ServerStatus::type_name(), typenames::SERVER_STATUS);
        assert_eq!(Task::type_name(), typenames::TASK);
        assert_eq!(AgentState::type_name(), typenames::AGENT_STATE);
        assert_eq!(TaskOutput::type_name(), typenames::TASK_OUTPUT);
        assert_eq!(SystemMetric::type_name(), typenames::SYSTEM_METRIC);
    }

    #[test]
    fn llm_types_are_keyless() {
        // REQ-003: os 3 tipos LLM* (e ServerStatus) não têm @key no IDL.
        assert_eq!(LLMInferenceRequest::key_count(), 0);
        assert_eq!(LLMInferenceResult::key_count(), 0);
        assert_eq!(LLMInferenceError::key_count(), 0);
        assert!(LLMInferenceRequest::keys().is_empty());
        assert!(LLMInferenceResult::keys().is_empty());
        assert!(LLMInferenceError::keys().is_empty());
    }

    #[test]
    fn v4_keys_match_pragma_keylist() {
        // REQ-002: keys de Task / AgentState / TaskOutput / SystemMetric.
        let task_keys: Vec<_> = Task::keys().into_iter().map(|k| k.name).collect();
        assert!(
            task_keys.iter().any(|n| n == "task_id"),
            "Task keys: {task_keys:?}"
        );
        assert_eq!(Task::key_count(), 1);

        let agent_keys: Vec<_> = AgentState::keys().into_iter().map(|k| k.name).collect();
        assert!(agent_keys.iter().any(|n| n == "agent_id"), "{agent_keys:?}");
        assert_eq!(AgentState::key_count(), 1);

        let out_keys: Vec<_> = TaskOutput::keys().into_iter().map(|k| k.name).collect();
        assert!(out_keys.iter().any(|n| n == "task_id"), "{out_keys:?}");
        assert!(out_keys.iter().any(|n| n == "seq_num"), "{out_keys:?}");
        assert_eq!(TaskOutput::key_count(), 2);

        let metric_keys: Vec<_> = SystemMetric::keys().into_iter().map(|k| k.name).collect();
        assert!(metric_keys.iter().any(|n| n == "metric_name"));
        assert!(metric_keys.iter().any(|n| n == "component_id"));
        assert_eq!(SystemMetric::key_count(), 2);
    }

    #[test]
    fn idl_file_llm_structs_are_keyless_by_source() {
        // Parse do IDL canônico (sem depender só do runtime).
        let idl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../llama_cpp/dds/idl/OrchestratorDDS.idl" // crates/dds-contract -> src/llama_cpp
        ));
        for name in [
            "LLMInferenceRequest",
            "LLMInferenceResult",
            "LLMInferenceError",
        ] {
            let re =
                regex::Regex::new(&format!(r"(?s)struct\s+{name}\s*\{{(?P<body>.*?)\n\s*\}};"))
                    .unwrap();
            let body = &re.captures(idl).unwrap_or_else(|| panic!("struct {name}"))["body"];
            assert!(
                !body.contains("@key"),
                "{name} must be keyless in IDL, body had @key"
            );
        }
    }

    #[test]
    fn roundtrip_llm_request() {
        let sample = LLMInferenceRequest {
            request_id: "req-1".into(),
            task_id: "task-1".into(),
            agent_id: "agent-1".into(),
            model_name: "qwen3.5-0.8b".into(),
            messages_json: r#"[{"role":"user","content":"hi"}]"#.into(),
            temperature: 0.7,
            max_tokens: 64,
            stream: true,
            security_level: 0,
            provider_constraint: "LOCAL_ONLY".into(),
            created_at_ns: 123456789,
        };
        let bytes = CdrSerializer::serialize(&sample, CdrEncoding::Xcdr1).unwrap();
        let back: LLMInferenceRequest =
            CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
        assert_eq!(back.request_id, sample.request_id);
        assert_eq!(back.task_id, sample.task_id);
        assert_eq!(back.messages_json, sample.messages_json);
        assert_eq!(back.max_tokens, sample.max_tokens);
        assert_eq!(back.stream, sample.stream);
        assert_eq!(back.created_at_ns, sample.created_at_ns);
    }

    #[test]
    fn roundtrip_llm_result() {
        let sample = LLMInferenceResult {
            request_id: "req-1".into(),
            seq_num: 3,
            content: "hello".into(),
            is_final: true,
            finish_reason: 1,
            model_used: "qwen".into(),
            tokens_prompt: 10,
            tokens_completion: 5,
            emitted_at_ns: 99,
        };
        let bytes = CdrSerializer::serialize(&sample, CdrEncoding::Xcdr1).unwrap();
        let back: LLMInferenceResult =
            CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
        assert_eq!(back.seq_num, 3);
        assert_eq!(back.content, "hello");
        assert!(back.is_final);
    }

    #[test]
    fn roundtrip_llm_error() {
        let sample = LLMInferenceError {
            request_id: "req-1".into(),
            error_code: 503,
            error_message: "busy".into(),
            provider: "local".into(),
            retriable: true,
            emitted_at_ns: 1,
        };
        let bytes = CdrSerializer::serialize(&sample, CdrEncoding::Xcdr1).unwrap();
        let back: LLMInferenceError =
            CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
        assert_eq!(back.error_code, 503);
        assert!(back.retriable);
    }

    #[test]
    fn roundtrip_task() {
        let sample = Task {
            task_id: "t-1".into(),
            client_id: "c-1".into(),
            assigned_agent: "".into(),
            target_agent: "".into(),
            model_required: 0,
            model_name: "qwen".into(),
            messages_json: "[]".into(),
            temperature: 0.7,
            max_tokens: 128,
            stream: false,
            status: 0,
            priority: 5,
            created_at_ns: 1,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns: 0,
            retry_count: 0,
            finish_reason: "".into(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        };
        let bytes = CdrSerializer::serialize(&sample, CdrEncoding::Xcdr1).unwrap();
        let back: Task = CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
        assert_eq!(back.task_id, "t-1");
        assert_eq!(back.priority, 5);
        assert_eq!(back.max_tokens, 128);
    }

    #[test]
    fn roundtrip_agent_state() {
        let sample = AgentState {
            agent_id: "a-1".into(),
            hostname: "host".into(),
            model: "qwen".into(),
            specialization: "TEXT".into(),
            slots_total: 4,
            slots_busy: 1,
            vram_total_mb: 24000,
            vram_used_mb: 8000,
            ema_latency_ms: 12.5,
            completed_total: 10,
            failed_total: 0,
            health: 2,
            last_update_ns: 100,
            uptime_seconds: 60,
        };
        let bytes = CdrSerializer::serialize(&sample, CdrEncoding::Xcdr1).unwrap();
        let back: AgentState = CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
        assert_eq!(back.agent_id, "a-1");
        assert_eq!(back.slots_total, 4);
        assert!((back.ema_latency_ms - 12.5).abs() < 0.01);
    }

    #[test]
    fn roundtrip_task_output() {
        let sample = TaskOutput {
            task_id: "t-1".into(),
            seq_num: 0,
            content: "tok".into(),
            is_final: false,
            finish_reason: 0,
            agent_id: "a-1".into(),
            token_count: 1,
            emitted_at_ns: 7,
        };
        let bytes = CdrSerializer::serialize(&sample, CdrEncoding::Xcdr1).unwrap();
        let back: TaskOutput = CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
        assert_eq!(back.seq_num, 0);
        assert_eq!(back.content, "tok");
    }
}
