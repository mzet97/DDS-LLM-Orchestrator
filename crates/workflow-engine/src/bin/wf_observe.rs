//! T-900-M2-04/V04 — observador shadow de decisões (feature `dds`).
//!
//! Assina `Workflow.Intent` + `Routing.Decision` (via `DdsWorkflowSpace`) e
//! `Tasks` (via `DataSpace`, APENAS leitura) e:
//! - valida cada decisão contra a intenção correspondente (política default);
//! - imprime uma linha JSON por veredito;
//! - no fim, reporta contadores — o gate V04 exige `tasks_seen == 0`
//!   publicadas PELO CONJUNTO (o worker não publica; o observador conta o
//!   que circula no domínio de teste isolado).
//!
//! NÃO publica nada: observador puro (M2 é shadow).

use dds_contract::workflow::dds_llm_workflow_v1::WorkflowIntent;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::workflow_space::{
    dds_impl::DdsWorkflowSpace, WorkflowSpaceApi, WorkflowSpaceLimits,
};
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use workflow_engine::decision::{validate, DecisionPolicy, Verdict};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut domain = 211u32;
    let mut timeout_s = 30u64;
    let mut expect = 1usize;
    let mut calibration_pin = String::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--domain" => domain = args.next().unwrap().parse()?,
            "--timeout-s" => timeout_s = args.next().unwrap().parse()?,
            "--expect" => expect = args.next().unwrap().parse()?,
            "--calibration-pin" => calibration_pin = args.next().unwrap(),
            other => anyhow::bail!("argumento desconhecido: {other}"),
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;
    rt.block_on(run(domain, timeout_s, expect, calibration_pin))
}

async fn run(
    domain: u32,
    timeout_s: u64,
    expect: usize,
    calibration_pin: String,
) -> anyhow::Result<()> {
    let space = DdsWorkflowSpace::new(domain, WorkflowSpaceLimits::default())?;
    // Tasks: leitura num domínio de teste isolado — conta o que CIRCULA
    // (worker não publica; se algo publicar, o gate V04 falha).
    let tasks_space = DataSpace::new(domain, DataSpace::STRENGTH_CLIENT)?;

    let mut intents = space.subscribe_intents();
    let mut decisions = space.subscribe_decisions();
    let mut tasks = tasks_space.subscribe_tasks();

    #[derive(Default)]
    struct Counters {
        intents: u64,
        decisions: u64,
        accepted: u64,
        errors: u64,
        rejected: u64,
        tasks_seen: u64,
        shadow_only: u64,
    }
    let mut intent_by_request: HashMap<String, Arc<WorkflowIntent>> = HashMap::new();
    let mut c = Counters::default();

    let mut policy = DecisionPolicy::default();
    if !calibration_pin.is_empty() {
        policy.calibration_pin = Some(calibration_pin);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);
    loop {
        let mut progress = false;
        tokio::select! {
            i = intents.next() => {
                if let Some(intent) = i {
                    progress = true;
                    c.intents += 1;
                    intent_by_request.insert(intent.request_id.to_string(), intent);
                }
            }
            d = decisions.next() => {
                if let Some(decision) = d {
                    progress = true;
                    c.decisions += 1;
                    let verdict = match intent_by_request.get(&decision.request_id.to_string()) {
                        Some(intent) => validate(&decision, intent, &policy, None),
                        None => Verdict::Reject {
                            reason: workflow_engine::RejectReason::Correlation,
                            detail: "decisão sem intenção correspondente".into(),
                        },
                    };
                    match &verdict {
                        Verdict::Accept { shadow_only, .. } => {
                            c.accepted += 1;
                            if *shadow_only {
                                c.shadow_only += 1;
                            }
                        }
                        Verdict::Error { .. } => {
                            c.errors += 1;
                        }
                        Verdict::Reject { .. } => {
                            c.rejected += 1;
                        }
                    }
                    let line = serde_json::json!({
                        "decision_id": decision.decision_id.to_string(),
                        "verdict": verdict_name(&verdict),
                        "reason": verdict_detail(&verdict),
                    });
                    println!("{line}");
                }
            }
            t = tasks.next() => {
                if t.is_some() {
                    progress = true;
                    c.tasks_seen += 1;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
        if c.decisions as usize >= expect {
            // dá uma janela pequena para Tasks "vazarem" (se alguém publicar)
            tokio::time::sleep(Duration::from_millis(750)).await;
            break;
        }
        if !progress && tokio::time::Instant::now() >= deadline {
            break;
        }
    }
    let stats = serde_json::json!({
        "intents": c.intents, "decisions": c.decisions, "accepted": c.accepted,
        "errors": c.errors, "rejected": c.rejected, "tasks_seen": c.tasks_seen,
        "shadow_only": c.shadow_only,
    });
    println!("{stats}");
    let ok = c.decisions as usize >= expect && c.tasks_seen == 0;
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

fn verdict_name(v: &Verdict) -> &'static str {
    match v {
        Verdict::Accept { .. } => "accept",
        Verdict::Error { .. } => "error",
        Verdict::Reject { .. } => "reject",
    }
}

fn verdict_detail(v: &Verdict) -> String {
    match v {
        Verdict::Accept {
            required_capabilities,
            priority,
            shadow_only,
            ..
        } => format!(
            "required={required_capabilities:?} priority={priority} shadow_only={shadow_only}"
        ),
        Verdict::Error { reason_code } => format!("reason_code={reason_code}"),
        Verdict::Reject { reason, detail } => format!("{reason}: {detail}"),
    }
}
