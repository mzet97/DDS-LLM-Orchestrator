//! Testes do contrato v4 — tipos de plataforma (WF-3).
//!
//! Cobre os 10 tipos adicionados ao `OrchestratorV4.idl` em 2026-07-17
//! (antes existiam só no `dds_types.py` Python, sem fonte IDL):
//! QoSRoutingProfile, ContextSnapshot, ContextUpdate, ToolCallRequest,
//! ExecutionTraceEvent, SecurityPolicySnapshot, SecurityPolicyUpdate,
//! QoSMetric, QoSViolation, DiscoveryEvent.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-contract --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use cyclonedds::{CdrDeserializer, CdrEncoding, CdrSerializer, DdsType};
use dds_contract::generated::dds_llm_orchestrator::*;
use dds_contract::typenames;

fn roundtrip<T>(sample: &T)
where
    T: DdsType + PartialEq + std::fmt::Debug + Clone,
{
    let bytes = CdrSerializer::serialize(sample, CdrEncoding::Xcdr1).unwrap();
    let back: T = CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
    assert_eq!(*sample, back);
}

#[test]
fn platform_typenames_match_python() {
    assert_eq!(
        QoSRoutingProfile::type_name(),
        typenames::QOS_ROUTING_PROFILE
    );
    assert_eq!(ContextSnapshot::type_name(), typenames::CONTEXT_SNAPSHOT);
    assert_eq!(ContextUpdate::type_name(), typenames::CONTEXT_UPDATE);
    assert_eq!(ToolCallRequest::type_name(), typenames::TOOL_CALL_REQUEST);
    assert_eq!(
        ExecutionTraceEvent::type_name(),
        typenames::EXECUTION_TRACE_EVENT
    );
    assert_eq!(
        SecurityPolicySnapshot::type_name(),
        typenames::SECURITY_POLICY_SNAPSHOT
    );
    assert_eq!(
        SecurityPolicyUpdate::type_name(),
        typenames::SECURITY_POLICY_UPDATE
    );
    assert_eq!(QoSMetric::type_name(), typenames::QOS_METRIC);
    assert_eq!(QoSViolation::type_name(), typenames::QOS_VIOLATION);
    assert_eq!(DiscoveryEvent::type_name(), typenames::DISCOVERY_EVENT);
}

#[test]
fn platform_keys_match_python() {
    let one =
        |k: Vec<cyclonedds::KeyDescriptor>| k.iter().map(|d| d.name.clone()).collect::<Vec<_>>();

    assert_eq!(QoSRoutingProfile::key_count(), 1);
    assert_eq!(one(QoSRoutingProfile::keys()), ["profile_id"]);

    assert_eq!(ContextSnapshot::key_count(), 1);
    assert_eq!(one(ContextSnapshot::keys()), ["context_id"]);

    assert_eq!(ContextUpdate::key_count(), 1);
    assert_eq!(one(ContextUpdate::keys()), ["context_id"]);

    assert_eq!(ToolCallRequest::key_count(), 1);
    assert_eq!(one(ToolCallRequest::keys()), ["call_id"]);

    assert_eq!(ExecutionTraceEvent::key_count(), 2);
    assert_eq!(one(ExecutionTraceEvent::keys()), ["trace_id", "seq_num"]);

    assert_eq!(SecurityPolicySnapshot::key_count(), 1);
    assert_eq!(one(SecurityPolicySnapshot::keys()), ["policy_id"]);

    assert_eq!(SecurityPolicyUpdate::key_count(), 1);
    assert_eq!(one(SecurityPolicyUpdate::keys()), ["policy_id"]);

    assert_eq!(QoSMetric::key_count(), 1);
    assert_eq!(one(QoSMetric::keys()), ["metric_id"]);

    assert_eq!(QoSViolation::key_count(), 1);
    assert_eq!(one(QoSViolation::keys()), ["violation_id"]);

    assert_eq!(DiscoveryEvent::key_count(), 1);
    assert_eq!(one(DiscoveryEvent::keys()), ["event_id"]);
}

/// `(type_info_blob, member_ids_blob)` do idlc para um tipo com metadata.
type MetadataBlobs = Option<(&'static [u8], &'static [u8])>;

#[test]
fn all_contract_types_have_type_metadata_blobs() {
    // Os 4 tipos v4 originais + os 10 novos + os 4 LLM carregam os blobs do idlc.
    let checks: [(&str, MetadataBlobs); 14] = [
        ("Task", Task::type_metadata_blobs()),
        ("AgentState", AgentState::type_metadata_blobs()),
        ("TaskOutput", TaskOutput::type_metadata_blobs()),
        ("SystemMetric", SystemMetric::type_metadata_blobs()),
        (
            "QoSRoutingProfile",
            QoSRoutingProfile::type_metadata_blobs(),
        ),
        ("ContextSnapshot", ContextSnapshot::type_metadata_blobs()),
        ("ContextUpdate", ContextUpdate::type_metadata_blobs()),
        ("ToolCallRequest", ToolCallRequest::type_metadata_blobs()),
        (
            "ExecutionTraceEvent",
            ExecutionTraceEvent::type_metadata_blobs(),
        ),
        (
            "SecurityPolicySnapshot",
            SecurityPolicySnapshot::type_metadata_blobs(),
        ),
        (
            "SecurityPolicyUpdate",
            SecurityPolicyUpdate::type_metadata_blobs(),
        ),
        ("QoSMetric", QoSMetric::type_metadata_blobs()),
        ("QoSViolation", QoSViolation::type_metadata_blobs()),
        ("DiscoveryEvent", DiscoveryEvent::type_metadata_blobs()),
    ];
    for (name, blobs) in checks {
        let (info, map) = blobs.unwrap_or_else(|| panic!("{name} sem blobs de metadata"));
        assert!(!info.is_empty(), "{name}: TYPE_INFO vazio");
        assert!(!map.is_empty(), "{name}: TYPE_MAP vazio");
    }
}

#[test]
fn roundtrip_platform_types_xcdr1() {
    roundtrip(&QoSRoutingProfile {
        profile_id: "GLOBAL".into(),
        version: 3,
        profile_name: "QoS_Balanced".into(),
        preferred_agent_prefix: "gpu".into(),
        allowed_agent_prefixes_json: "[\"gpu\",\"cpu\"]".into(),
        weights_json: "{\"lat\":0.5}".into(),
        fallback_after_ms: 1500,
        centroid_score: 0.42,
        explanation_json: "{}".into(),
        timestamp_ns: 123,
    });

    roundtrip(&ContextSnapshot {
        context_id: "ctx-1".into(),
        client_id: "cli-1".into(),
        session_id: "sess-1".into(),
        messages_json: "[]".into(),
        metadata_json: "{}".into(),
        security_level: 1,
        created_at_ns: 10,
        updated_at_ns: 20,
        ttl_seconds: 3600,
    });

    roundtrip(&ContextUpdate {
        context_id: "ctx-1".into(),
        update_type: 2,
        messages_delta_json: "[+]".into(),
        metadata_delta_json: "{+}".into(),
        updated_at_ns: 30,
    });

    roundtrip(&ToolCallRequest {
        call_id: "call-1".into(),
        request_id: "req-1".into(),
        tool_name: "filesystem.read".into(),
        arguments_json: "{\"path\":\"/tmp/x\"}".into(),
        security_level: 2,
        status: 0,
        result_json: "{}".into(),
        error_message: String::new(),
        created_at_ns: 40,
        completed_at_ns: 0,
    });

    roundtrip(&ExecutionTraceEvent {
        trace_id: "tr-1".into(),
        seq_num: 7,
        event_type: 1,
        task_id: "t-1".into(),
        request_id: "r-1".into(),
        agent_id: "a-1".into(),
        component_id: "c-1".into(),
        component_type: 2,
        payload_json: "{\"k\":1}".into(),
        timestamp_ns: 50,
    });

    roundtrip(&SecurityPolicySnapshot {
        policy_id: "pol-1".into(),
        version: 9,
        policy_json: "{\"rules\":[]}".into(),
        published_by: "policy-engine".into(),
        timestamp_ns: 60,
    });

    roundtrip(&SecurityPolicyUpdate {
        policy_id: "pol-1".into(),
        previous_version: 8,
        new_version: 9,
        operation: "add_rule".into(),
        rule_delta_json: "{\"+\":1}".into(),
        published_by: "policy-engine".into(),
        timestamp_ns: 70,
    });

    roundtrip(&QoSMetric {
        metric_id: "m-1".into(),
        metric_name: "deadline_missed".into(),
        component: "orchestrator".into(),
        value: 1_000_000_000_000,
        delta: 5,
        window_ms: 1000,
        timestamp_ns: 80,
        experiment_id: "E1".into(),
        run_id: "r-01".into(),
        qos_profile: "QoS_Balanced".into(),
        load_level: "low".into(),
        network_condition: "clean".into(),
        payload_level: "small".into(),
        replication_idx: 2,
        warmup: true,
    });

    roundtrip(&QoSViolation {
        violation_id: "v-1".into(),
        violation_type: "deadline_missed".into(),
        topic_name: "TaskOutput".into(),
        component: "agent".into(),
        entity_kind: "WRITER".into(),
        affected_entity: "agent-01".into(),
        severity: "ERROR".into(),
        details_json: "{}".into(),
        timestamp_ns: 90,
        experiment_id: "E1".into(),
        run_id: "r-01".into(),
        qos_profile: "QoS_Critical".into(),
        load_level: "high".into(),
        network_condition: "degraded".into(),
        payload_level: "large".into(),
        replication_idx: 0,
        warmup: false,
    });

    roundtrip(&DiscoveryEvent {
        event_id: "e-1".into(),
        event_type: "PUBLICATION_MATCHED".into(),
        topic_name: "Tasks".into(),
        local_entity: "reader-1".into(),
        remote_entity: "writer-2".into(),
        count_change: 1,
        timestamp_ns: 100,
    });
}
