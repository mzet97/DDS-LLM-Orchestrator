//! T-900-M1-02 — wire do WorkflowPlanning com runtime real (feature `dds`).
//!
//! Provas: typename qualificado, chaves por tópico, roundtrip XCDR1 dos tipos
//! com sequences aninhadas (inéditas no V4) e injeção dos blobs XTypes do idlc
//! C (`dds_type_metadata`). Interop de PROCESSO real Rust↔Python é T-900-M1-04
//! (V02/V03); aqui é a superfície local.
//!
//! Nota de API: strings BOUNDED geram `DdsString` (TryFrom) e sequences geram
//! `DdsBoundedSequence<T, N>` (from_vec, checa o bound) — diferente do V4,
//! que só tem strings ilimitadas (`String`).

#![cfg(feature = "dds")]

use cyclonedds::{CdrDeserializer, CdrEncoding, CdrSerializer, DdsType};
use dds_contract::workflow::dds_llm_workflow_v1::{
    AgentCapabilities, InputRef, PlanNode, ProfileProbability, RoutingDecision, WorkflowIntent,
    WorkflowPlan, WorkflowState,
};
use dds_contract::{topics, typenames, workflow_topics};

fn s(v: &str) -> cyclonedds::DdsString {
    v.try_into().expect("DdsString válida")
}

fn intent_sample() -> WorkflowIntent {
    WorkflowIntent {
        request_id: s("req-m1-02"),
        workflow_id: s("wf-m1-02"),
        client_id: s("cli-1"),
        schema_version: 1,
        intent_revision: 1,
        command: 0, // IC_SUBMIT
        expected_plan_revision: 0,
        prompt: s("Audit this Rust service and create tests"),
        created_at_ns: 1_760_000_000_000_000_001,
        deadline_ns: 1_760_000_600_000_000_001,
        security_context_ref: s("ctx://tenant-a/policy-3"),
        policy_revision: 7,
        questions_revision: 2,
        capability_catalog_revision: 3,
        checkpoint_revision: s(&"ab".repeat(32)),
        max_tokens: 8192,
        max_cost_microunits: 1_500_000,
        budget_unit: s("MICRO_USD"),
        fallback_template_id: s(""),
        payload_hash: s(&"cd".repeat(32)),
    }
}

#[test]
fn workflow_typenames_match_idl_module() {
    assert_eq!(WorkflowIntent::type_name(), typenames::WORKFLOW_INTENT);
    assert_eq!(RoutingDecision::type_name(), typenames::ROUTING_DECISION);
    assert_eq!(
        AgentCapabilities::type_name(),
        typenames::AGENT_CAPABILITIES
    );
    assert_eq!(WorkflowPlan::type_name(), typenames::WORKFLOW_PLAN);
    assert_eq!(WorkflowState::type_name(), typenames::WORKFLOW_STATE);
    for tn in [
        typenames::WORKFLOW_INTENT,
        typenames::ROUTING_DECISION,
        typenames::AGENT_CAPABILITIES,
        typenames::WORKFLOW_PLAN,
        typenames::WORKFLOW_STATE,
    ] {
        assert!(tn.starts_with("dds_llm_workflow_v1::"));
    }
}

#[test]
fn workflow_keys_match_contract() {
    let intent: Vec<_> = WorkflowIntent::keys().into_iter().map(|k| k.name).collect();
    assert_eq!(intent, vec!["request_id"]);
    let decision: Vec<_> = RoutingDecision::keys()
        .into_iter()
        .map(|k| k.name)
        .collect();
    assert_eq!(decision, vec!["request_id", "decision_id"]);
    assert_eq!(RoutingDecision::key_count(), 2);
    let caps: Vec<_> = AgentCapabilities::keys()
        .into_iter()
        .map(|k| k.name)
        .collect();
    assert_eq!(caps, vec!["agent_id"]);
    let plan: Vec<_> = WorkflowPlan::keys().into_iter().map(|k| k.name).collect();
    assert_eq!(plan, vec!["workflow_id", "plan_revision"]);
    assert_eq!(WorkflowPlan::key_count(), 2);
    let state: Vec<_> = WorkflowState::keys().into_iter().map(|k| k.name).collect();
    assert_eq!(state, vec!["workflow_id", "plan_revision"]);
}

#[test]
fn workflow_topics_isolated_from_canonical_nineteen() {
    // REQ-900-003/012: o catálogo canônico de 19 permanece intocado; os cinco
    // tópicos da fase 900 são opt-in e não podem vazar para o inventário fixado.
    assert_eq!(topics::ALL.len(), 19);
    for t in workflow_topics::ALL {
        assert!(
            !topics::ALL.contains(&t),
            "tópico {t} vazou para topics::ALL"
        );
    }
    assert_eq!(workflow_topics::ALL.len(), 5);
}

#[test]
fn roundtrip_intent_xcdr1() {
    let sample = intent_sample();
    for encoding in [CdrEncoding::Xcdr1, CdrEncoding::Xcdr2] {
        let bytes = CdrSerializer::serialize(&sample, encoding).unwrap();
        let back: WorkflowIntent = CdrDeserializer::deserialize(&bytes, encoding).unwrap();
        assert_eq!(back.request_id.to_string(), "req-m1-02");
        assert_eq!(back.schema_version, 1);
        assert_eq!(back.command, 0);
        assert_eq!(back.max_cost_microunits, sample.max_cost_microunits);
        assert_eq!(
            back.prompt.to_string(),
            "Audit this Rust service and create tests"
        );
    }
}

#[test]
fn roundtrip_plan_with_nested_sequences_xcdr1() {
    let node = PlanNode {
        node_id: s("node-b"),
        kind: 0, // NK_INFERENCE
        required_capabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![
            s("rust_refactor"),
            s("unit_testing"),
        ])
        .unwrap(),
        model_id: s("gemma-4-12b-it"),
        target_agent_id: s(""),
        prompt_template: s("Summarize the audit report"),
        dependencies: cyclonedds::DdsBoundedSequence::from_vec(vec![s("node-a")]).unwrap(),
        inputs: cyclonedds::DdsBoundedSequence::from_vec(vec![InputRef {
            producer_node_id: s("node-a"),
            output_name: s("report"),
        }])
        .unwrap(),
        output_names: cyclonedds::DdsBoundedSequence::from_vec(vec![s("summary")]).unwrap(),
        max_attempts: 2,
        deadline_ns: 1_760_000_500_000_000_001,
        max_tokens: 1024,
        max_output_bytes: 65_536,
        security_context_ref: s("ctx://tenant-a/policy-3"),
        priority: 5,
        side_effect_key: s(""),
    };
    let plan = WorkflowPlan {
        workflow_id: s("wf-m1-02"),
        plan_revision: 1,
        schema_version: 1,
        request_id: s("req-m1-02"),
        intent_revision: 1,
        effective_decision_id: s("dec-0001"),
        policy_revision: 7,
        capability_catalog_revision: 3,
        planner_id: s("static-template"),
        planner_revision: s(&"ef".repeat(32)),
        created_at_ns: 1_760_000_010_000_000_001,
        deadline_ns: 1_760_000_600_000_000_001,
        qos_profile: 1, // QI_CRITICAL
        nodes: cyclonedds::DdsBoundedSequence::from_vec(vec![node]).unwrap(),
        plan_hash: s(&"ab".repeat(32)),
    };
    let bytes = CdrSerializer::serialize(&plan, CdrEncoding::Xcdr1).unwrap();
    let back: WorkflowPlan = CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
    assert_eq!(back.nodes.len(), 1);
    assert_eq!(back.nodes.as_slice()[0].node_id.to_string(), "node-b");
    let caps = back.nodes.as_slice()[0].required_capabilities.to_vec();
    assert_eq!(caps[0].to_string(), "rust_refactor");
    assert_eq!(caps[1].to_string(), "unit_testing");
    let inputs = back.nodes.as_slice()[0].inputs.to_vec();
    assert_eq!(inputs[0].producer_node_id.to_string(), "node-a");
    assert_eq!(inputs[0].output_name.to_string(), "report");
    assert_eq!(back.qos_profile, 1);
    assert_eq!(back.plan_hash.to_string(), "ab".repeat(32));
}

#[test]
fn roundtrip_decision_with_probability_sequences_xcdr1() {
    let decision = RoutingDecision {
        request_id: s("req-m1-02"),
        decision_id: s("dec-0001"),
        workflow_id: s("wf-m1-02"),
        schema_version: 1,
        intent_revision: 1,
        producer_id: s("laya-worker-1"),
        model_id: s("convaiinnovations/laya"),
        checkpoint_revision: s(&"ab".repeat(32)),
        questions_revision: 2,
        policy_revision: 7,
        capability_catalog_revision: 3,
        qos_profile: 0, // QI_BALANCED
        qos_probabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![
            ProfileProbability {
                profile: 0,
                probability: 0.62,
            },
            ProfileProbability {
                profile: 1,
                probability: 0.38,
            },
        ])
        .unwrap(),
        capability_probabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![]).unwrap(),
        requires_planning_probability: 0.25,
        priority_score: 0.9,
        priority_probabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![0.05, 0.55, 0.40])
            .unwrap(),
        confidence: 0.71,
        answer_confidence: 0.66,
        calibration_revision: s(""),
        created_at_ns: 1_760_000_005_000_000_001,
        valid_until_ns: 1_760_000_300_000_000_001,
        decision_status: 0, // DS_OK
        reason_code: 0,     // RC_NONE
        evidence_digest: s(&"00".repeat(32)),
        payload_hash: s(&"11".repeat(32)),
    };
    let bytes = CdrSerializer::serialize(&decision, CdrEncoding::Xcdr1).unwrap();
    let back: RoutingDecision = CdrDeserializer::deserialize(&bytes, CdrEncoding::Xcdr1).unwrap();
    assert_eq!(back.qos_probabilities.len(), 2);
    let probs = back.qos_probabilities.to_vec();
    assert!((probs[0].probability - 0.62).abs() < 1e-12);
    assert_eq!(back.priority_probabilities.len(), 3);
    assert_eq!(back.decision_id.to_string(), "dec-0001");
}

#[test]
fn bounded_sequence_rejects_over_limit() {
    // REQ-900-014: rejeitar acima do limite, nunca truncar silenciosamente.
    let result: Result<cyclonedds::DdsBoundedSequence<cyclonedds::DdsString, 8>, _> =
        cyclonedds::DdsBoundedSequence::from_vec((0..9).map(|i| s(&format!("cap-{i}"))).collect());
    assert!(
        result.is_err(),
        "sequence com 9 itens deve exceder o bound 8"
    );
}

#[test]
fn workflow_type_metadata_blobs_injected_from_idlc_c() {
    // O build.rs injeta TYPE_INFO/TYPE_MAP do `WorkflowPlanning.c` (idlc C
    // 11.0.1) — sem isso, endpoints Rust anunciariam TypeIds derivados do
    // binding e o matching XTypes com C/Python divergiria (R-INTEROP).
    for (info, map) in [
        WorkflowIntent::type_metadata_blobs().unwrap(),
        RoutingDecision::type_metadata_blobs().unwrap(),
        AgentCapabilities::type_metadata_blobs().unwrap(),
        WorkflowPlan::type_metadata_blobs().unwrap(),
        WorkflowState::type_metadata_blobs().unwrap(),
    ] {
        assert!(!info.is_empty(), "TYPE_INFO_CDR vazio");
        assert!(!map.is_empty(), "TYPE_MAP_CDR vazio");
    }
}
