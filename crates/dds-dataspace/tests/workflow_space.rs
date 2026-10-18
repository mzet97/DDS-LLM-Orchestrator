//! T-900-M1-03 — aceites do WorkflowSpace (backend in-memory).
//!
//! Gate `not(dds)` como o `mock_parity.rs` (unificação de features do
//! `--workspace` troca os tipos mock pelos gerados). O backend DDS real é
//! exercitado pelo harness de interop T-900-M1-04.
//!
//! Aceite da tarefa: "Dados capacidade cheia, versão inválida e lag, quando
//! escritos/lidos, então erro tipado e limites cumpridos; snapshots/replay
//! não perdem estado silenciosamente."

#![cfg(not(feature = "dds"))]

use dds_contract::workflow::dds_llm_workflow_v1 as w;
use dds_dataspace::workflow_space::{
    InMemoryWorkflowSpace, WorkflowSpaceApi, WorkflowSpaceError, WorkflowSpaceLimits,
};
use std::sync::Arc;

fn intent(request_id: &str, rev: u32, hash: &str) -> w::WorkflowIntent {
    w::WorkflowIntent {
        request_id: request_id.into(),
        workflow_id: "wf-1".into(),
        client_id: "cli-1".into(),
        schema_version: 1,
        intent_revision: rev,
        command: 0,
        expected_plan_revision: 0,
        prompt: "p".into(),
        created_at_ns: 1,
        deadline_ns: 2,
        security_context_ref: "s".into(),
        policy_revision: 1,
        questions_revision: 1,
        capability_catalog_revision: 1,
        checkpoint_revision: "a".into(),
        max_tokens: 10,
        max_cost_microunits: 0,
        budget_unit: "u".into(),
        fallback_template_id: String::new(),
        payload_hash: hash.into(),
    }
}

fn decision(request_id: &str, decision_id: &str, hash: &str) -> w::RoutingDecision {
    w::RoutingDecision {
        request_id: request_id.into(),
        decision_id: decision_id.into(),
        workflow_id: "wf-1".into(),
        schema_version: 1,
        intent_revision: 1,
        producer_id: "laya-1".into(),
        model_id: "laya".into(),
        checkpoint_revision: "a".into(),
        questions_revision: 1,
        policy_revision: 1,
        capability_catalog_revision: 1,
        qos_profile: 0,
        qos_probabilities: vec![],
        capability_probabilities: vec![],
        requires_planning_probability: 0.1,
        priority_score: 1.0,
        priority_probabilities: vec![0.1, 0.8, 0.1],
        confidence: 0.7,
        answer_confidence: 0.6,
        calibration_revision: String::new(),
        created_at_ns: 1,
        valid_until_ns: 2,
        decision_status: 0,
        reason_code: 0,
        evidence_digest: "e".into(),
        payload_hash: hash.into(),
    }
}

fn caps(agent_id: &str, rev: u32) -> w::AgentCapabilities {
    w::AgentCapabilities {
        agent_id: agent_id.into(),
        schema_version: 1,
        announcement_revision: rev,
        capability_catalog_revision: 1,
        capabilities: vec!["rust_refactor".into()],
        model_ids: vec!["gemma".into()],
        tool_versions: vec![],
        adaptive_protocol_version: 1,
        security_context_ref: "s".into(),
        created_at_ns: 1,
        valid_until_ns: 2,
    }
}

fn plan(workflow_id: &str, rev: u32, hash: &str) -> w::WorkflowPlan {
    w::WorkflowPlan {
        workflow_id: workflow_id.into(),
        plan_revision: rev,
        schema_version: 1,
        request_id: "req-1".into(),
        intent_revision: 1,
        effective_decision_id: "dec-1".into(),
        policy_revision: 1,
        capability_catalog_revision: 1,
        planner_id: "static-template".into(),
        planner_revision: "r".into(),
        created_at_ns: 1,
        deadline_ns: 2,
        qos_profile: 0,
        nodes: vec![],
        plan_hash: hash.into(),
    }
}

fn state(workflow_id: &str, plan_rev: u32, state_rev: u32) -> w::WorkflowState {
    w::WorkflowState {
        workflow_id: workflow_id.into(),
        plan_revision: plan_rev,
        schema_version: 1,
        state_revision: state_rev,
        request_id: "req-1".into(),
        effective_decision_id: "dec-1".into(),
        plan_hash: "h".into(),
        status: 3, // WS_RUNNING
        reason_code: 0,
        nodes: vec![],
        created_at_ns: 1,
        updated_at_ns: state_rev as u64,
        expires_at_ns: 9,
        cancelled_by_request_id: String::new(),
        effective_qos_config_hash: "q".into(),
        fallback_template_id: String::new(),
    }
}

#[tokio::test]
async fn write_read_roundtrip_all_five_topics() {
    let space = InMemoryWorkflowSpace::default();
    space.write_intent(intent("req-1", 1, "h1")).await.unwrap();
    space
        .write_decision(decision("req-1", "dec-1", "h1"))
        .await
        .unwrap();
    space.write_capabilities(caps("agent-1", 1)).await.unwrap();
    space.write_plan(plan("wf-1", 1, "ph")).await.unwrap();
    space.write_state(state("wf-1", 1, 5)).await.unwrap();

    assert_eq!(
        space
            .read_intent("req-1")
            .await
            .unwrap()
            .unwrap()
            .payload_hash,
        "h1"
    );
    assert!(space
        .read_decision("req-1", "dec-1")
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        space
            .read_capabilities("agent-1")
            .await
            .unwrap()
            .unwrap()
            .announcement_revision,
        1
    );
    assert!(space.read_plan("wf-1", 1).await.unwrap().is_some());
    // chave composta: revisão diferente é instância diferente
    space.write_plan(plan("wf-1", 2, "ph2")).await.unwrap();
    assert_eq!(
        space.read_plan("wf-1", 2).await.unwrap().unwrap().plan_hash,
        "ph2"
    );
    assert!(space.read_state("wf-1", 1).await.unwrap().is_some());
    assert!(space.read_state("wf-1", 2).await.unwrap().is_none());
}

#[tokio::test]
async fn invalid_schema_version_is_typed_error_on_every_topic() {
    let space = InMemoryWorkflowSpace::default();
    let mut i = intent("req-1", 1, "h");
    i.schema_version = 2;
    assert_eq!(
        space.write_intent(i).await.unwrap_err(),
        WorkflowSpaceError::InvalidSchemaVersion {
            topic: "Workflow.Intent",
            expected: 1,
            got: 2
        }
    );
    let mut d = decision("req-1", "dec-1", "h");
    d.schema_version = 0;
    assert!(matches!(
        space.write_decision(d).await,
        Err(WorkflowSpaceError::InvalidSchemaVersion { got: 0, .. })
    ));
    let mut c = caps("a", 1);
    c.schema_version = 7;
    assert!(matches!(
        space.write_capabilities(c).await,
        Err(WorkflowSpaceError::InvalidSchemaVersion { got: 7, .. })
    ));
    let mut p = plan("wf", 1, "h");
    p.schema_version = 3;
    assert!(matches!(
        space.write_plan(p).await,
        Err(WorkflowSpaceError::InvalidSchemaVersion { got: 3, .. })
    ));
    let mut s = state("wf", 1, 1);
    s.schema_version = 9;
    assert!(matches!(
        space.write_state(s).await,
        Err(WorkflowSpaceError::InvalidSchemaVersion { got: 9, .. })
    ));
    // Nada foi aceito:
    assert!(space.instance_counts().values().all(|&n| n == 0));
}

#[tokio::test]
async fn capacity_full_is_typed_error() {
    let limits = WorkflowSpaceLimits {
        max_instances_intent: 2,
        ..WorkflowSpaceLimits::default()
    };
    let space = InMemoryWorkflowSpace::new(limits);
    space.write_intent(intent("r1", 1, "h")).await.unwrap();
    space.write_intent(intent("r2", 1, "h")).await.unwrap();
    let err = space.write_intent(intent("r3", 1, "h")).await.unwrap_err();
    assert_eq!(
        err,
        WorkflowSpaceError::CapacityFull {
            topic: "Workflow.Intent",
            have: 2,
            limit: 2
        }
    );
    // Substituir chave existente NÃO conta como nova instância.
    space.write_intent(intent("r1", 2, "h")).await.unwrap();
    assert_eq!(space.instance_counts()["Workflow.Intent"], 2);
}

#[tokio::test]
async fn stale_revisions_are_typed_lag_errors() {
    let space = InMemoryWorkflowSpace::default();
    // State: state_revision regressiva
    space.write_state(state("wf", 1, 5)).await.unwrap();
    let err = space.write_state(state("wf", 1, 3)).await.unwrap_err();
    assert_eq!(
        err,
        WorkflowSpaceError::Stale {
            topic: "Workflow.State",
            key: "wf|1".into(),
            current: 5,
            received: 3,
        }
    );
    // Capabilities: announcement_revision regressiva
    space.write_capabilities(caps("a", 4)).await.unwrap();
    assert!(matches!(
        space.write_capabilities(caps("a", 2)).await,
        Err(WorkflowSpaceError::Stale {
            current: 4,
            received: 2,
            ..
        })
    ));
    // Intent: intent_revision regressiva
    space.write_intent(intent("r", 2, "h")).await.unwrap();
    assert!(matches!(
        space.write_intent(intent("r", 1, "h")).await,
        Err(WorkflowSpaceError::Stale {
            current: 2,
            received: 1,
            ..
        })
    ));
    // Estado não regrediu:
    assert_eq!(
        space
            .read_state("wf", 1)
            .await
            .unwrap()
            .unwrap()
            .state_revision,
        5
    );
}

#[tokio::test]
async fn same_revision_different_payload_is_conflict() {
    let space = InMemoryWorkflowSpace::default();
    space.write_intent(intent("r", 1, "hash-a")).await.unwrap();
    assert_eq!(
        space
            .write_intent(intent("r", 1, "hash-b"))
            .await
            .unwrap_err(),
        WorkflowSpaceError::Conflict {
            topic: "Workflow.Intent",
            key: "r".into(),
            revision: 1,
        }
    );
    // Decision imutável por (request, decision): payload diferente = conflito
    space
        .write_decision(decision("r", "d1", "h1"))
        .await
        .unwrap();
    assert!(matches!(
        space.write_decision(decision("r", "d1", "h2")).await,
        Err(WorkflowSpaceError::Conflict { .. })
    ));
    // State: mesma revisão, conteúdo diferente = conflito
    space.write_state(state("wf", 1, 7)).await.unwrap();
    let mut s2 = state("wf", 1, 7);
    s2.status = 4; // mesmo state_revision, conteúdo diferente
    assert!(matches!(
        space.write_state(s2).await,
        Err(WorkflowSpaceError::Conflict { revision: 7, .. })
    ));
    // Plan imutável por (workflow, revisão): hash diferente = conflito
    space.write_plan(plan("wf", 1, "ph1")).await.unwrap();
    assert!(matches!(
        space.write_plan(plan("wf", 1, "ph2")).await,
        Err(WorkflowSpaceError::Conflict { .. })
    ));
}

#[tokio::test]
async fn idempotent_replay_is_accepted_at_least_once() {
    let space = InMemoryWorkflowSpace::default();
    space.write_intent(intent("r", 1, "h")).await.unwrap();
    // Replay idêntico (at-least-once): OK, sem erro, sem instância nova.
    space.write_intent(intent("r", 1, "h")).await.unwrap();
    space.write_decision(decision("r", "d", "h")).await.unwrap();
    space.write_decision(decision("r", "d", "h")).await.unwrap();
    space.write_state(state("wf", 1, 5)).await.unwrap();
    space.write_state(state("wf", 1, 5)).await.unwrap();
    assert_eq!(space.instance_counts()["Workflow.Intent"], 1);
    assert_eq!(space.instance_counts()["Routing.Decision"], 1);
    assert_eq!(space.instance_counts()["Workflow.State"], 1);
}

#[tokio::test]
async fn snapshot_restore_loses_nothing_silently() {
    let space = InMemoryWorkflowSpace::default();
    space.write_intent(intent("r1", 1, "h")).await.unwrap();
    space.write_intent(intent("r2", 1, "h")).await.unwrap();
    space
        .write_decision(decision("r1", "d1", "h"))
        .await
        .unwrap();
    space.write_capabilities(caps("a1", 3)).await.unwrap();
    space.write_plan(plan("wf1", 1, "ph")).await.unwrap();
    space.write_state(state("wf1", 1, 2)).await.unwrap();

    let snap = space.snapshot();
    let restored = InMemoryWorkflowSpace::default();
    restored.restore(snap).unwrap();

    // Counts conferem 1:1 — nada se perdeu.
    for (topic, count) in space.instance_counts() {
        assert_eq!(restored.instance_counts()[topic], count, "{topic}");
    }
    assert_eq!(
        restored
            .read_intent("r2")
            .await
            .unwrap()
            .unwrap()
            .request_id,
        "r2"
    );
    assert_eq!(
        restored
            .read_state("wf1", 1)
            .await
            .unwrap()
            .unwrap()
            .state_revision,
        2
    );

    // Replay de snapshot sobre espaço JÁ populado com divergência = erro
    // tipado (não sobrescreve silenciosamente).
    let other = InMemoryWorkflowSpace::default();
    other.write_state(state("wf1", 1, 9)).await.unwrap();
    let mut conflicting = other.snapshot();
    conflicting.states = vec![Arc::new(state("wf1", 1, 4))]; // atrasada
    assert!(matches!(
        other.restore(conflicting),
        Err(WorkflowSpaceError::Stale { .. })
    ));
}

#[tokio::test]
async fn cache_byte_budget_is_enforced_with_typed_error() {
    // Orçamento determinístico: 1,5x o custo da primeira amostra — a segunda
    // (chave distinta) necessariamente estoura.
    let probe = InMemoryWorkflowSpace::default();
    probe.write_intent(intent("r1", 1, "h")).await.unwrap();
    let one = probe.bytes_used();
    assert!(one > 0);
    let limits = WorkflowSpaceLimits {
        cache_byte_budget: one + one / 2,
        ..WorkflowSpaceLimits::default()
    };
    let space = InMemoryWorkflowSpace::new(limits);
    space.write_intent(intent("r1", 1, "h")).await.unwrap();
    let err = space.write_intent(intent("r2", 1, "h")).await.unwrap_err();
    assert!(matches!(
        err,
        WorkflowSpaceError::CacheBudgetExceeded { budget, .. } if budget == one + one / 2
    ));
    assert_eq!(space.instance_counts()["Workflow.Intent"], 1);
}

#[tokio::test]
async fn subscribe_delivers_only_accepted_samples() {
    use futures::StreamExt;
    let space = InMemoryWorkflowSpace::default();
    let mut stream = space.subscribe_states();
    space.write_state(state("wf", 1, 1)).await.unwrap();
    let mut bad = state("wf", 1, 0); // mesma chave, revisão menor
    bad.state_revision = 0;
    // write aceita? revisão 0 < 1 → Stale, NÃO vai para a stream.
    assert!(space.write_state(bad).await.is_err());
    space.write_state(state("wf", 1, 2)).await.unwrap();

    let first = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .expect("timeout aguardando amostra")
        .expect("stream encerrada");
    assert_eq!(first.state_revision, 1);
    let second = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .expect("timeout aguardando 2a amostra")
        .expect("stream encerrada");
    assert_eq!(second.state_revision, 2);
}
