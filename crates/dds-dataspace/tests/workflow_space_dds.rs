//! T-900-M1-03 — smoke do backend DDS real do WorkflowSpace.
//!
//! Um participante publica `Workflow.Intent` e um SEGUNDO participante lê de
//! volta (TransientLocal/KeepLast(1)): valida criação de entidades com os
//! perfis `qos::profiles::workflow`, tipagem do wire e leitura por
//! `read_latest`. Domínio isolado (mesma prática do t808_dds). O interop
//! Rust↔Python completo é T-900-M1-04 (V02/V03).

#![cfg(feature = "dds")]

use dds_contract::workflow::dds_llm_workflow_v1::WorkflowIntent;
use dds_dataspace::workflow_space::{
    dds_impl::DdsWorkflowSpace, WorkflowSpaceApi, WorkflowSpaceLimits,
};
use std::time::Duration;

const DOMAIN: u32 = 209;

fn sample(request_id: &str) -> WorkflowIntent {
    WorkflowIntent {
        request_id: request_id.try_into().unwrap(),
        workflow_id: "wf-dds-1".try_into().unwrap(),
        client_id: "cli-1".try_into().unwrap(),
        schema_version: 1,
        intent_revision: 1,
        command: 0,
        expected_plan_revision: 0,
        prompt: "probe m1-03".try_into().unwrap(),
        created_at_ns: 1,
        deadline_ns: 2,
        security_context_ref: "s".try_into().unwrap(),
        policy_revision: 1,
        questions_revision: 1,
        capability_catalog_revision: 1,
        checkpoint_revision: "a".try_into().unwrap(),
        max_tokens: 10,
        max_cost_microunits: 0,
        budget_unit: "u".try_into().unwrap(),
        fallback_template_id: "".try_into().unwrap(),
        payload_hash: "h".try_into().unwrap(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dds_workflow_space_publishes_and_reads_intent_across_participants() {
    let writer = DdsWorkflowSpace::new(DOMAIN, WorkflowSpaceLimits::default()).unwrap();
    let reader = DdsWorkflowSpace::new(DOMAIN, WorkflowSpaceLimits::default()).unwrap();

    writer.write_intent(sample("req-m1-03")).await.unwrap();

    // Descoberta + TransientLocal: re-tenta leitura por até 5 s.
    let mut got: Option<String> = None;
    for _ in 0..50 {
        if let Some(i) = reader.read_intent("req-m1-03").await.unwrap() {
            got = Some(i.request_id.to_string());
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(got.as_deref(), Some("req-m1-03"));

    // Fail-closed: versão inválida NEM chega a publicar.
    let mut bad = sample("req-bad");
    bad.schema_version = 2;
    assert!(writer.write_intent(bad).await.is_err());
    assert!(reader.read_intent("req-bad").await.unwrap().is_none());
}
