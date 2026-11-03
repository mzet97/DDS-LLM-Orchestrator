//! T-900-M2-02 — matriz de testes do validator de decisão (REQ-900-004).
//!
//! Gate `not(dds)` (mocks do dds-contract; unificação de features do
//! workspace troca os tipos). Construtores espelham o worker Python.

#![cfg(not(feature = "dds"))]

use dds_contract::workflow::dds_llm_workflow_v1::{RoutingDecision, WorkflowIntent};
use workflow_engine::decision::{validate, DecisionPolicy, RejectReason, Verdict};

fn intent() -> WorkflowIntent {
    WorkflowIntent {
        request_id: "req-1".into(),
        workflow_id: "wf-1".into(),
        client_id: "cli".into(),
        schema_version: 1,
        intent_revision: 1,
        command: 0,
        expected_plan_revision: 0,
        prompt: "audit this rust service and create tests".into(),
        created_at_ns: 1_000,
        deadline_ns: 10_000,
        security_context_ref: "ctx".into(),
        policy_revision: 7,
        questions_revision: 1,
        capability_catalog_revision: 1,
        checkpoint_revision: "ab".repeat(32),
        max_tokens: 100,
        max_cost_microunits: 0,
        budget_unit: "MICRO_USD".into(),
        fallback_template_id: String::new(),
        payload_hash: "cd".repeat(32),
    }
}

fn decision_ok() -> RoutingDecision {
    RoutingDecision {
        request_id: "req-1".into(),
        decision_id: "dec-req-1".into(),
        workflow_id: "wf-1".into(),
        schema_version: 1,
        intent_revision: 1,
        producer_id: "laya-worker-1".into(),
        model_id: "fake:rules-baseline-v1".into(),
        checkpoint_revision: "ab".repeat(32),
        questions_revision: 1,
        policy_revision: 7,
        capability_catalog_revision: 1,
        qos_profile: 1, // QI_CRITICAL (argmax)
        qos_probabilities: full_probs().into_iter().map(|(pr, v)| p(pr, v)).collect(),
        capability_probabilities: vec![],
        requires_planning_probability: 0.2,
        priority_score: 1.5,
        priority_probabilities: vec![0.1, 0.7, 0.2],
        confidence: 0.55,
        answer_confidence: 0.55,
        calibration_revision: String::new(),
        created_at_ns: 2_000,
        valid_until_ns: 8_000,
        decision_status: 0,
        reason_code: 0,
        evidence_digest: String::new(),
        payload_hash: "ef".repeat(32),
    }
}

fn full_probs() -> Vec<(i32, f64)> {
    vec![(0, 0.10), (1, 0.60), (2, 0.10), (3, 0.10), (4, 0.10)]
}

#[test]
fn accept_ok_decision_and_derive() {
    let mut d = decision_ok();
    d.qos_probabilities = full_probs().into_iter().map(|(pr, v)| p(pr, v)).collect();
    d.capability_probabilities = vec![
        cap("rust_refactor", 0.91),
        cap("unit_testing", 0.30), // abaixo do limiar 0.5
    ];
    let v = validate(&d, &intent(), &DecisionPolicy::default(), Some(5_000));
    match v {
        Verdict::Accept {
            required_capabilities,
            qos_profile,
            priority,
            shadow_only,
            below_confidence_threshold,
        } => {
            assert_eq!(required_capabilities, vec!["rust_refactor".to_string()]);
            assert_eq!(qos_profile, 1);
            assert_eq!(priority, 5); // argmax NORMAL
            assert!(shadow_only); // calibration vazia (M2 é shadow)
            assert!(!below_confidence_threshold);
        }
        other => panic!("esperava Accept, veio {other:?}"),
    }
}

#[test]
fn priority_argmax_maps_low_normal_high() {
    let mut d = decision_ok();
    d.qos_probabilities = full_probs().into_iter().map(|(pr, v)| p(pr, v)).collect();
    d.priority_probabilities = vec![0.8, 0.1, 0.1];
    let v = validate(&d, &intent(), &DecisionPolicy::default(), Some(5_000));
    assert!(matches!(v, Verdict::Accept { priority: 1, .. }));
    d.priority_probabilities = vec![0.1, 0.1, 0.8];
    let v = validate(&d, &intent(), &DecisionPolicy::default(), Some(5_000));
    assert!(matches!(v, Verdict::Accept { priority: 10, .. }));
}

#[test]
fn rejects_schema_version() {
    let mut d = decision_ok();
    d.schema_version = 2;
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::SchemaVersion,
            ..
        }
    ));
}

#[test]
fn rejects_correlation_mismatch() {
    let mut d = decision_ok();
    d.request_id = "outra".into();
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::Correlation,
            ..
        }
    ));
}

#[test]
fn rejects_revision_pin_mismatch() {
    let mut d = decision_ok();
    d.questions_revision = 2;
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::RevisionPin,
            ..
        }
    ));
    let mut d2 = decision_ok();
    d2.checkpoint_revision = "ff".repeat(32);
    assert!(matches!(
        validate(&d2, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::RevisionPin,
            ..
        }
    ));
}

#[test]
fn rejects_validity_window_and_expiry() {
    // skew 0 p/ valores pequenos de teste (o default 5e9 ns anularia a janela).
    let no_skew = DecisionPolicy {
        skew_tolerance_ns: 0,
        ..DecisionPolicy::default()
    };
    let mut d = decision_ok();
    d.valid_until_ns = 20_000; // deadline 10_000 + skew 5_000
    assert!(matches!(
        validate(&d, &intent(), &no_skew, None),
        Verdict::Reject {
            reason: RejectReason::ValidityWindow,
            ..
        }
    ));
    let mut d2 = decision_ok();
    d2.valid_until_ns = 4_000; // expirada p/ now=9_000 (skew 5s não salva: now-skew=... usa sub)
    assert!(matches!(
        validate(&d2, &intent(), &no_skew, Some(10_000)),
        Verdict::Reject {
            reason: RejectReason::ValidityWindow,
            ..
        }
    ));
}

#[test]
fn well_formed_error_is_verdict_error() {
    let mut d = decision_ok();
    d.decision_status = 1;
    d.reason_code = 2; // RC_MODEL_UNAVAILABLE
    match validate(&d, &intent(), &DecisionPolicy::default(), None) {
        Verdict::Error { reason_code } => assert_eq!(reason_code, 2),
        other => panic!("esperava Error, veio {other:?}"),
    }
}

#[test]
fn rejects_status_reason_inconsistency() {
    let mut d = decision_ok();
    d.reason_code = 3; // OK com reason
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::StatusReason,
            ..
        }
    ));
    let mut d2 = decision_ok();
    d2.decision_status = 1;
    d2.reason_code = 0; // ERROR sem reason
    assert!(matches!(
        validate(&d2, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::StatusReason,
            ..
        }
    ));
}

#[test]
fn rejects_broken_qos_distribution() {
    let mut d = decision_ok();
    d.qos_probabilities = vec![p(0, 0.5)]; // soma != 1
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::QosDistribution,
            ..
        }
    ));
    let mut d2 = decision_ok();
    d2.qos_probabilities = vec![p(1, 0.6), p(1, 0.4)]; // duplicata
    assert!(matches!(
        validate(&d2, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::QosDistribution,
            ..
        }
    ));
    let mut d3 = decision_ok();
    d3.qos_profile = 0; // não é o argmax
    d3.qos_probabilities = full_probs().into_iter().map(|(pr, v)| p(pr, v)).collect();
    assert!(matches!(
        validate(&d3, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::QosDistribution,
            ..
        }
    ));
}

#[test]
fn rejects_priority_problems() {
    let mut d = decision_ok();
    d.priority_score = 2.5;
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::PriorityInvalid,
            ..
        }
    ));
    let mut d2 = decision_ok();
    d2.priority_probabilities = vec![0.5, 0.5];
    assert!(matches!(
        validate(&d2, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::PriorityInvalid,
            ..
        }
    ));
}

#[test]
fn rejects_unknown_and_duplicate_capabilities() {
    let mut d = decision_ok();
    d.capability_probabilities = vec![cap("magic_wand", 0.9)];
    assert!(matches!(
        validate(&d, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::CapabilityUnknown(1),
            ..
        }
    ));
    let mut d2 = decision_ok();
    d2.capability_probabilities = vec![cap("research", 0.9), cap("research", 0.6)];
    assert!(matches!(
        validate(&d2, &intent(), &DecisionPolicy::default(), None),
        Verdict::Reject {
            reason: RejectReason::CapabilityDuplicate,
            ..
        }
    ));
}

#[test]
fn confidence_gate_enforcement_is_policy() {
    let mut d = decision_ok();
    d.confidence = 0.30;
    let mut policy = DecisionPolicy {
        confidence_threshold: Some(0.5),
        enforce_confidence: false,
        ..DecisionPolicy::default()
    };
    // observador: informativo
    assert!(matches!(
        validate(&d, &intent(), &policy, None),
        Verdict::Accept {
            below_confidence_threshold: true,
            ..
        }
    ));
    policy.enforce_confidence = true;
    assert!(matches!(
        validate(&d, &intent(), &policy, None),
        Verdict::Reject {
            reason: RejectReason::ConfidenceBelowThreshold(0.30),
            ..
        }
    ));
}

// helpers ------------------------------------------------------------

fn p(
    profile: i32,
    probability: f64,
) -> dds_contract::workflow::dds_llm_workflow_v1::ProfileProbability {
    dds_contract::workflow::dds_llm_workflow_v1::ProfileProbability {
        profile,
        probability,
    }
}
fn cap(id: &str, prob: f64) -> dds_contract::workflow::dds_llm_workflow_v1::CapabilityProbability {
    dds_contract::workflow::dds_llm_workflow_v1::CapabilityProbability {
        capability_id: id.into(),
        probability: prob,
        answer_confidence: 0.5,
    }
}

/// T-900-A3: pin de calibração governa shadow_only (contracts: sem
/// calibração validada não há ativação).
#[test]
fn calibration_pin_governs_shadow_mode() {
    let mut d = decision_ok();
    d.qos_probabilities = full_probs().into_iter().map(|(pr, v)| p(pr, v)).collect();
    d.calibration_revision = "a".repeat(64);

    // sem pin na política: TUDO é shadow (mesmo com revisão preenchida)
    let v = validate(&d, &intent(), &DecisionPolicy::default(), None);
    assert!(matches!(
        v,
        Verdict::Accept {
            shadow_only: true,
            ..
        }
    ));

    // com pin DIVERGENTE: shadow
    let pol = DecisionPolicy {
        calibration_pin: Some("b".repeat(64)),
        ..DecisionPolicy::default()
    };
    let v = validate(&d, &intent(), &pol, None);
    assert!(matches!(
        v,
        Verdict::Accept {
            shadow_only: true,
            ..
        }
    ));

    // com pin CASANDO: sai do shadow (execução pode ativar)
    let pol = DecisionPolicy {
        calibration_pin: Some("a".repeat(64)),
        ..DecisionPolicy::default()
    };
    let v = validate(&d, &intent(), &pol, None);
    assert!(matches!(
        v,
        Verdict::Accept {
            shadow_only: false,
            ..
        }
    ));
}
