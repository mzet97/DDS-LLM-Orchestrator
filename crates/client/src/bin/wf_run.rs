//! `wf-run`: driver de workload integralmente em Rust (sem harness Python).
//!
//! Encadeia etapas via Tasks DDS reais (`DdsClientDds`): o driver decide a
//! progressão (sequência, barreira fork-join, controle serial), cada etapa é
//! reivindicada pelo agente Rust e inferida no backend DDS real. Emite um
//! registro JSON por run para análise posterior (Python só analisa).
//!
//! Uso: `wf-run -- --domain 78 --workload seq_chain_v1 --entry "..."
//!   --prompts-dir benchmarks/orchestration/prompts --model qwen3.5-0.8b
//!   --workflow-id dds-w1 --out record.json`

#[path = "wf/assembly.rs"]
mod assembly;
#[path = "wf/record.rs"]
mod record;

use assembly::{ANALYSIS, CORRECTNESS, REVIEW, SECURITY};
use client::dds_impl::DdsClientDds;
use client::{ClientConfig, DdsClient};
use record::{emit, RunMeta, StageRec};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

struct Cfg {
    domain: u32,
    workload: String,
    entry: String,
    prompts_dir: String,
    model: String,
    timeout_ms: u64,
    meta: RunMeta,
}

fn parse_args() -> Result<Cfg, String> {
    let args: Vec<String> = std::env::args().collect();
    let meta = RunMeta {
        workflow_id: String::from("dds-w1"),
        workload: String::new(),
        entry: String::new(),
        model: String::from("qwen3.5-0.8b"),
        out: String::from("wf-record.json"),
    };
    let mut c = Cfg {
        domain: 78,
        workload: String::new(),
        entry: String::new(),
        prompts_dir: String::from("benchmarks/orchestration/prompts"),
        model: meta.model.clone(),
        timeout_ms: 120_000,
        meta,
    };
    // Preenche Cfg e espelha os campos do registro.
    let mut i = 1;
    while i < args.len() {
        let val = |i: usize| -> Result<String, String> {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} exige valor", args[i]))
        };
        match args[i].as_str() {
            "--domain" => c.domain = val(i)?.parse().map_err(|e| format!("domain: {e}"))?,
            "--workload" => c.workload = val(i)?,
            "--entry" => c.entry = val(i)?,
            "--prompts-dir" => c.prompts_dir = val(i)?,
            "--model" => c.model = val(i)?,
            "--workflow-id" => c.meta.workflow_id = val(i)?,
            "--out" => c.meta.out = val(i)?,
            "--timeout-ms" => {
                c.timeout_ms = val(i)?.parse().map_err(|e| format!("timeout: {e}"))?
            }
            other => return Err(format!("argumento desconhecido: {other}")),
        }
        i += 2;
    }
    if c.workload.is_empty() || c.entry.is_empty() {
        return Err("--workload e --entry são obrigatórios".to_string());
    }
    c.meta.workload.clone_from(&c.workload);
    c.meta.entry.clone_from(&c.entry);
    c.meta.model.clone_from(&c.model);
    Ok(c)
}

fn client_config(client_id: String, cfg: &Cfg) -> ClientConfig {
    ClientConfig {
        client_id,
        dds_domain: cfg.domain,
        timeout_ms: cfg.timeout_ms,
    }
}

async fn submit_stage(
    client: &DdsClientDds,
    helper: &DdsClient,
    model: &str,
    messages_json: &str,
) -> Result<(String, String, u64), Box<dyn std::error::Error>> {
    let task = helper.create_task(model, messages_json, 5, false);
    let r = client.submit(task).await?;
    if !r.success {
        return Err(format!("task {} sem sucesso", r.task_id).into());
    }
    Ok((r.task_id, r.content, r.latency_ms))
}

async fn run_stage(
    client: &DdsClientDds,
    helper: &DdsClient,
    model: &str,
    stage: &'static str,
    messages_json: &str,
    expects: Vec<String>,
) -> Result<StageRec, Box<dyn std::error::Error>> {
    let started_ms = ms_now();
    let (task_id, content, latency_ms) = submit_stage(client, helper, model, messages_json).await?;
    Ok(StageRec {
        stage,
        task_id,
        content,
        expects,
        latency_ms,
        started_ms,
        finished_ms: ms_now(),
    })
}

async fn run_seq(cfg: &Cfg) -> Result<(), Box<dyn std::error::Error>> {
    let config = client_config(format!("{}-wf", cfg.meta.workflow_id), cfg);
    let helper = DdsClient::new(config.clone());
    let client = DdsClientDds::new(config)?;
    let pa = assembly::load_prompt(&cfg.prompts_dir, "seq_A_analyst_v1.txt")?;
    let pb = assembly::load_prompt(&cfg.prompts_dir, "seq_B_reviewer_v1.txt")?;
    let pc = assembly::load_prompt(&cfg.prompts_dir, "seq_C_consolidator_v1.txt")?;
    let a = run_stage(
        &client,
        &helper,
        &cfg.model,
        "A",
        &assembly::build_messages(&pa, &cfg.entry, &[]),
        vec![],
    )
    .await?;
    let b = run_stage(
        &client,
        &helper,
        &cfg.model,
        "B",
        &assembly::build_messages(&pb, &cfg.entry, &[(ANALYSIS, a.content.clone())]),
        vec![a.content.clone()],
    )
    .await?;
    let c = run_stage(
        &client,
        &helper,
        &cfg.model,
        "C",
        &assembly::build_messages(
            &pc,
            &cfg.entry,
            &[(ANALYSIS, a.content.clone()), (REVIEW, b.content.clone())],
        ),
        vec![a.content.clone(), b.content.clone()],
    )
    .await?;
    println!("WF_RECORD {}", emit(&cfg.meta, &[a, b, c])?);
    Ok(())
}

async fn run_fork(cfg: &Cfg, serial: bool) -> Result<(), Box<dyn std::error::Error>> {
    let workload = if serial {
        "fork_join_serial_v1"
    } else {
        "fork_join_v1"
    };
    let helper = DdsClient::new(client_config(format!("{}-wf", cfg.meta.workflow_id), cfg));
    let client = Arc::new(DdsClientDds::new(client_config(
        format!("{}-wf", cfg.meta.workflow_id),
        cfg,
    ))?);
    let pa = assembly::load_prompt(&cfg.prompts_dir, "fork_A_correctness_v1.txt")?;
    let pb = assembly::load_prompt(&cfg.prompts_dir, "fork_B_security_v1.txt")?;
    let pc = assembly::load_prompt(&cfg.prompts_dir, "fork_C_consolidator_v1.txt")?;
    let ma = assembly::build_messages(&pa, &cfg.entry, &[]);
    let mb = assembly::build_messages(&pb, &cfg.entry, &[]);
    let model = cfg.model.clone();
    let (a, b) = if serial {
        let h = &helper;
        let a = run_stage(&client, h, &model, "A", &ma, vec![]).await?;
        let b = run_stage(&client, h, &model, "B", &mb, vec![]).await?;
        (a, b)
    } else {
        let (ca, cb) = (Arc::clone(&client), Arc::clone(&client));
        let (mb2, model2) = (mb.clone(), model.clone());
        let ha_h = DdsClient::new(client_config("fork-a".to_string(), cfg));
        let hb_h = DdsClient::new(client_config("fork-b".to_string(), cfg));
        let ha = tokio::spawn(async move {
            run_stage(&ca, &ha_h, &model, "A", &ma, vec![])
                .await
                .map_err(|e| e.to_string())
        });
        let hb = tokio::spawn(async move {
            run_stage(&cb, &hb_h, &model2, "B", &mb2, vec![])
                .await
                .map_err(|e| e.to_string())
        });
        let join_err = |e: tokio::task::JoinError| -> Box<dyn std::error::Error> { e.into() };
        let boxed = |e: String| -> Box<dyn std::error::Error> { e.into() };
        (
            ha.await.map_err(join_err)?.map_err(boxed)?,
            hb.await.map_err(join_err)?.map_err(boxed)?,
        )
    };
    // Barreira: C só é montado após A e B concluídos.
    let c = run_stage(
        &client,
        &helper,
        &cfg.model,
        "C",
        &assembly::build_messages(
            &pc,
            &cfg.entry,
            &[
                (CORRECTNESS, a.content.clone()),
                (SECURITY, b.content.clone()),
            ],
        ),
        vec![a.content.clone(), b.content.clone()],
    )
    .await?;
    let meta = RunMeta {
        workflow_id: cfg.meta.workflow_id.clone(),
        workload: workload.to_string(),
        entry: cfg.entry.clone(),
        model: cfg.model.clone(),
        out: cfg.meta.out.clone(),
    };
    println!("WF_RECORD {}", emit(&meta, &[a, b, c])?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = parse_args()
        .map_err(|e| format!("uso: wf-run -- --domain N --workload W --entry E [...]\n{e}"))?;
    match cfg.workload.as_str() {
        "seq_chain_v1" => run_seq(&cfg).await?,
        "fork_join_v1" => run_fork(&cfg, false).await?,
        "fork_join_serial_v1" => run_fork(&cfg, true).await?,
        w => return Err(format!("workload desconhecido: {w}").into()),
    }
    Ok(())
}
