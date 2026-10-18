//! T-900-M1-04 — harness de interop Rust↔Python dos 5 tópicos da fase 900.
//!
//! Uso:
//!   workflow-interop --mode pub  --domain N [--count 1] [--hold-s 0]
//!   workflow-interop --mode sub  --domain N --timeout-s T
//!   workflow-interop --mode late --domain N --timeout-s T   (subscriber tardio V03)
//!
//! `pub` publica uma amostra de cada tópico (valores determinísticos
//! "v02-<topic>") e imprime uma linha JSON por publicação. `sub` assina os
//! cinco tópicos via `DdsWorkflowSpace` (admissão de contrato aplicada) e
//! imprime `{"topic":..., ...campos-chave}` por amostra aceita; termina 0
//! quando viu os 5 tópicos, ou 1 no timeout. `late` igual a `sub`, mas é
//! iniciado DEPOIS do publicador (V03 — TransientLocal).
//!
//! O driver completo (V02 bidirecional, mismatch deliberado, late joiner,
//! regressão dos 19) está em
//! `workers/laya-worker/harness/run_interop.py`.

use anyhow::Result;
use dds_contract::workflow::dds_llm_workflow_v1 as w;
use dds_dataspace::workflow_space::{
    dds_impl::DdsWorkflowSpace, WorkflowSpaceApi, WorkflowSpaceLimits,
};
use futures::StreamExt;
use std::time::Duration;

#[derive(Debug)]
struct Cli {
    mode: String,
    domain: u32,
    count: u32,
    timeout_s: u64,
    hold_s: u64,
}

fn parse_args() -> Cli {
    let mut mode = "sub".to_string();
    let mut domain = 211u32;
    let mut count = 1u32;
    let mut timeout_s = 10u64;
    let mut hold_s = 0u64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mode" => mode = args.next().expect("valor de --mode"),
            "--domain" => domain = args.next().expect("valor").parse().expect("u32"),
            "--count" => count = args.next().expect("valor").parse().expect("u32"),
            "--timeout-s" => timeout_s = args.next().expect("valor").parse().expect("u64"),
            "--hold-s" => hold_s = args.next().expect("valor").parse().expect("u64"),
            other => panic!("argumento desconhecido: {other}"),
        }
    }
    Cli {
        mode,
        domain,
        count,
        timeout_s,
        hold_s,
    }
}

fn s(v: &str) -> cyclonedds::DdsString {
    v.try_into().unwrap()
}

fn intent(id: &str) -> w::WorkflowIntent {
    w::WorkflowIntent {
        request_id: s(id),
        workflow_id: s("wf-v02"),
        client_id: s("harness"),
        schema_version: 1,
        intent_revision: 1,
        command: 0,
        expected_plan_revision: 0,
        prompt: s("interop probe m1-04"),
        created_at_ns: 1,
        deadline_ns: 2,
        security_context_ref: s("ctx"),
        policy_revision: 1,
        questions_revision: 1,
        capability_catalog_revision: 1,
        checkpoint_revision: s("a"),
        max_tokens: 10,
        max_cost_microunits: 0,
        budget_unit: s("MICRO_USD"),
        fallback_template_id: s(""),
        payload_hash: s("ph"),
    }
}

fn decision(id: &str) -> w::RoutingDecision {
    w::RoutingDecision {
        request_id: s(id),
        decision_id: s("dec-v02"),
        workflow_id: s("wf-v02"),
        schema_version: 1,
        intent_revision: 1,
        producer_id: s("harness"),
        model_id: s("laya"),
        checkpoint_revision: s("a"),
        questions_revision: 1,
        policy_revision: 1,
        capability_catalog_revision: 1,
        qos_profile: 1,
        qos_probabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![w::ProfileProbability {
            profile: 1,
            probability: 0.9,
        }])
        .unwrap(),
        capability_probabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![]).unwrap(),
        requires_planning_probability: 0.2,
        priority_score: 1.5,
        priority_probabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![0.1, 0.7, 0.2])
            .unwrap(),
        confidence: 0.8,
        answer_confidence: 0.7,
        calibration_revision: s(""),
        created_at_ns: 1,
        valid_until_ns: 2,
        decision_status: 0,
        reason_code: 0,
        evidence_digest: s("e"),
        payload_hash: s("dh"),
    }
}

fn caps(agent: &str) -> w::AgentCapabilities {
    w::AgentCapabilities {
        agent_id: s(agent),
        schema_version: 1,
        announcement_revision: 1,
        capability_catalog_revision: 1,
        capabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![
            s("rust_refactor"),
            s("unit_testing"),
        ])
        .unwrap(),
        model_ids: cyclonedds::DdsBoundedSequence::from_vec(vec![s("gemma-4-12b-it")]).unwrap(),
        tool_versions: cyclonedds::DdsBoundedSequence::from_vec(vec![]).unwrap(),
        adaptive_protocol_version: 1,
        security_context_ref: s("ctx"),
        created_at_ns: 1,
        valid_until_ns: 11,
    }
}

fn plan(wf: &str) -> w::WorkflowPlan {
    let node = w::PlanNode {
        node_id: s("node-a"),
        kind: 0,
        required_capabilities: cyclonedds::DdsBoundedSequence::from_vec(vec![s("rust_refactor")])
            .unwrap(),
        model_id: s("gemma-4-12b-it"),
        target_agent_id: s(""),
        prompt_template: s("probe"),
        dependencies: cyclonedds::DdsBoundedSequence::from_vec(vec![]).unwrap(),
        inputs: cyclonedds::DdsBoundedSequence::from_vec(vec![]).unwrap(),
        output_names: cyclonedds::DdsBoundedSequence::from_vec(vec![s("out")]).unwrap(),
        max_attempts: 2,
        deadline_ns: 3,
        max_tokens: 64,
        max_output_bytes: 1024,
        security_context_ref: s("ctx"),
        priority: 5,
        side_effect_key: s(""),
    };
    w::WorkflowPlan {
        workflow_id: s(wf),
        plan_revision: 1,
        schema_version: 1,
        request_id: s("req-v02"),
        intent_revision: 1,
        effective_decision_id: s("dec-v02"),
        policy_revision: 1,
        capability_catalog_revision: 1,
        planner_id: s("static-template"),
        planner_revision: s("r"),
        created_at_ns: 1,
        deadline_ns: 3,
        qos_profile: 1,
        nodes: cyclonedds::DdsBoundedSequence::from_vec(vec![node]).unwrap(),
        plan_hash: s("planhash"),
    }
}

fn state(wf: &str) -> w::WorkflowState {
    w::WorkflowState {
        workflow_id: s(wf),
        plan_revision: 1,
        schema_version: 1,
        state_revision: 1,
        request_id: s("req-v02"),
        effective_decision_id: s("dec-v02"),
        plan_hash: s("planhash"),
        status: 3,
        reason_code: 0,
        nodes: cyclonedds::DdsBoundedSequence::from_vec(vec![]).unwrap(),
        created_at_ns: 1,
        updated_at_ns: 1,
        expires_at_ns: 99,
        cancelled_by_request_id: s(""),
        effective_qos_config_hash: s("qoshash"),
        fallback_template_id: s(""),
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let cli = parse_args();
    let space = DdsWorkflowSpace::new(cli.domain, WorkflowSpaceLimits::default())?;

    match cli.mode.as_str() {
        "pub" => {
            for i in 0..cli.count {
                space
                    .write_intent(intent(&format!("v02-intent-{i}")))
                    .await?;
                println!(r#"{{"published":"Workflow.Intent","request_id":"v02-intent-{i}"}}"#);
                space
                    .write_decision(decision(&format!("v02-intent-{i}")))
                    .await?;
                println!(
                    r#"{{"published":"Routing.Decision","request_id":"v02-intent-{i}","decision_id":"dec-v02"}}"#
                );
                space.write_capabilities(caps("v02-agent")).await?;
                println!(r#"{{"published":"Agent.Capabilities","agent_id":"v02-agent"}}"#);
                space.write_plan(plan("v02-wf")).await?;
                println!(
                    r#"{{"published":"Workflow.Plan","workflow_id":"v02-wf","plan_revision":1}}"#
                );
                space.write_state(state("v02-wf")).await?;
                println!(
                    r#"{{"published":"Workflow.State","workflow_id":"v02-wf","state_revision":1}}"#
                );
            }
            if cli.hold_s > 0 {
                // V03: mantém o writer vivo para o late joiner receber
                // TransientLocal.
                tokio::time::sleep(Duration::from_secs(cli.hold_s)).await;
            }
            Ok(())
        }
        "sub" | "late" => {
            let mut intents = space.subscribe_intents();
            let mut decisions = space.subscribe_decisions();
            let mut capabilities = space.subscribe_capabilities();
            let mut plans = space.subscribe_plans();
            let mut states = space.subscribe_states();
            let mut seen: Vec<&'static str> = Vec::new();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(cli.timeout_s);
            loop {
                let next = tokio::select! {
                    i = intents.next() => {
                        if let Some(x) = i {
                            println!(r#"{{"topic":"Workflow.Intent","request_id":"{}"}}"#, x.request_id);
                            Some("Workflow.Intent")
                        } else { None }
                    }
                    d = decisions.next() => {
                        if let Some(x) = d {
                            println!(r#"{{"topic":"Routing.Decision","request_id":"{}","decision_id":"{}"}}"#, x.request_id, x.decision_id);
                            Some("Routing.Decision")
                        } else { None }
                    }
                    c = capabilities.next() => {
                        if let Some(x) = c {
                            println!(r#"{{"topic":"Agent.Capabilities","agent_id":"{}","announcement_revision":{}}}"#, x.agent_id, x.announcement_revision);
                            Some("Agent.Capabilities")
                        } else { None }
                    }
                    p = plans.next() => {
                        if let Some(x) = p {
                            println!(r#"{{"topic":"Workflow.Plan","workflow_id":"{}","plan_revision":{},"nodes":{}}}"#, x.workflow_id, x.plan_revision, x.nodes.len());
                            Some("Workflow.Plan")
                        } else { None }
                    }
                    st = states.next() => {
                        if let Some(x) = st {
                            println!(r#"{{"topic":"Workflow.State","workflow_id":"{}","plan_revision":{},"state_revision":{}}}"#, x.workflow_id, x.plan_revision, x.state_revision);
                            Some("Workflow.State")
                        } else { None }
                    }
                    _ = tokio::time::sleep_until(deadline) => break,
                };
                if let Some(t) = next {
                    if !seen.contains(&t) {
                        seen.push(t);
                    }
                    if seen.len() == 5 {
                        println!(r#"{{"complete":true}}"#);
                        return Ok(());
                    }
                }
            }
            eprintln!("timeout: viu apenas {seen:?}");
            std::process::exit(1);
        }
        other => anyhow::bail!("modo desconhecido: {other}"),
    }
}
