//! wf-run — runtime E2E do engine da fase 900 (T-900-E2E-B2).
//!
//! Uso (domínio único; agentes com --adaptive-domain igual ao --domain):
//!   wf-run --domain 216 --planner llm --llama-url http://127.0.0.1:8901 \
//!         --journal-dir /tmp/wf-run-journal [--timeout-s 300]
//!
//! Pipeline real: Workflow.Intent → Routing.Decision (worker Laya) →
//! validator/mapper (workflow-engine) → planner (direct|llm) → DAG validate →
//! Plan/State → laya1- Tasks (outbox) → agentes --adaptive executam (engine
//! http/dds/mock) → Task DONE → on_done → WorkflowState DONE.

use anyhow::Result;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::workflow_space::{
    dds_impl::DdsWorkflowSpace, WorkflowSpaceApi, WorkflowSpaceLimits,
};
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use workflow_engine::engine::{DirectTemplatePlanner, Planner};
use workflow_engine::runtime::{EngineRuntime, RuntimeConfig};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let mut domain = 216u32;
    let mut planner_kind = String::from("direct");
    let mut llama_url = String::new();
    let mut journal_dir = String::from("/tmp/wf-run-journal");
    let mut timeout_s = 300u64;
    let mut expect = 1usize;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--domain" => domain = args.next().unwrap().parse()?,
            "--planner" => planner_kind = args.next().unwrap(),
            "--llama-url" => llama_url = args.next().unwrap(),
            "--journal-dir" => journal_dir = args.next().unwrap(),
            "--timeout-s" => timeout_s = args.next().unwrap().parse()?,
            "--expect" => expect = args.next().unwrap().parse()?,
            other => anyhow::bail!("argumento desconhecido: {other}"),
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Tasks em Exclusive Ownership: o engine publica PENDING com strength de
    // CLIENTE (10) para que o claim do agente (100) vença a arbitragem — mesmo
    // papel do reaper legado.
    let dataspace = Arc::new(DataSpace::new(domain, DataSpace::STRENGTH_CLIENT)?);
    let wspace = Arc::new(DdsWorkflowSpace::new(
        domain,
        WorkflowSpaceLimits::default(),
    )?);
    let journal = workflow_engine::journal::Journal::open(std::path::Path::new(&journal_dir))?;
    let planner: Box<dyn Planner> = match planner_kind.as_str() {
        "direct" => Box::new(DirectTemplatePlanner),
        "llm" => Box::new(workflow_engine::planner_http::LlmHttpPlanner::new(
            &llama_url,
        )),
        other => anyhow::bail!("--planner {other} inválido (direct|llm)"),
    };
    let mut rt = EngineRuntime::new(
        Arc::clone(&dataspace),
        Arc::clone(&wspace),
        journal,
        planner,
        RuntimeConfig::default(),
    );

    let mut intents = wspace.subscribe_intents();
    let mut decisions = wspace.subscribe_decisions();
    let mut tasks = dataspace.subscribe_tasks();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);
    let mut done_count = 0usize;
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    loop {
        let mut activity = false;
        tokio::select! {
            i = intents.next() => {
                if let Some(intent) = i {
                    activity = true;
                    tracing::info!(wf = %intent.workflow_id.to_string(), "intent recebida");
                    rt.on_intent(&intent).await?;
                }
            }
            d = decisions.next() => {
                if let Some(decision) = d {
                    activity = true;
                    let ok = rt.on_decision(&decision).await?;
                    tracing::info!(wf = %decision.workflow_id.to_string(), admitted = ok, "decisão processada");
                }
            }
            t = tasks.next() => {
                if t.is_some() {
                    activity = true; // watcher no tick
                }
            }
            _ = tick.tick() => {}
        }
        let published = rt.tick_scheduler().await?;
        let handled = rt.tick_watch_tasks().await?;
        let in_doubt = rt.tick_timeouts().await?;
        if published + handled + in_doubt > 0 {
            activity = true;
            tracing::info!(published, handled, in_doubt, "tick");
        }
        let finished = rt
            .workflows_summary()
            .iter()
            .filter(|(_, term, fail)| *term && fail.is_none())
            .count();
        if finished > done_count {
            done_count = finished;
        }
        if done_count >= expect {
            tracing::info!(done_count, "workflows concluídos");
            for (wf, term, fail) in rt.workflows_summary() {
                println!(
                    "{}",
                    serde_json::json!({"workflow": wf, "terminal": term,
                                       "failed": fail.unwrap_or_default()})
                );
            }
            return Ok(());
        }
        if !activity && tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "timeout: {} workflows concluídos (esperava {expect})",
                done_count
            );
        }
    }
}
