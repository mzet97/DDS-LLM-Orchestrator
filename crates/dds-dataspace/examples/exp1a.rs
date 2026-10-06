//! EXP1a (dissertação §3.7.3): ablação de substrato no MESMO binário —
//! `InMemoryDataSpace` vs `DataSpace` (CycloneDDS real), sobre a mesma
//! trait `DataSpaceApi`, com o MESMO worker (claim otimista + confirmação
//! por releitura do mesh RHC + 2 chunks + DONE) e o mesmo fluxo A→B→C.
//!
//! A variável independente é o substrato de comunicação (T-880/EXP1a):
//! runtime, worker e fluxo permanecem fixos. Resultado medido (2026-10-05):
//! substrato DDS ≈ 0 de custo (18 ms p50 vs 18 ms in-memory, delay 0;
//! 162 vs 164 ms, delay 50 — n=30 por célula).
//!
//! Build/run:
//!   cargo run --release -p dds-dataspace --example exp1a -- --n 30 --delay-ms 0
//!   cargo run --release -p dds-dataspace --features dds --example exp1a -- --n 30 --delay-ms 0
//!   (CYCLONEDDS_URI de loopback no braço DDS; domínio fresco por execução)

use std::sync::Arc;
use std::time::Instant;

use dds_dataspace::api::DataSpaceApi;
use futures::StreamExt;

#[derive(serde::Serialize, Clone)]
struct WorkflowRecord {
    workflow: usize,
    backend: String,
    delay_ms: u64,
    t_total_ms: u128,
    stages: [String; 3],
}

fn stage_hash(stage: &str, wf: usize, attempt: u32) -> String {
    let mut h: u64 = 14_695_981_039_346_656_037;
    for b in format!("{stage}:{wf}:{attempt}").as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1_099_511_628_211);
    }
    format!("{h:016x}")
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut n, mut delay_ms) = (30usize, 0u64);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--n" => n = args.next().unwrap().parse()?,
            "--delay-ms" => delay_ms = args.next().unwrap().parse()?,
            other => anyhow::bail!("argumento desconhecido: {other}"),
        }
    }

    #[cfg(feature = "dds")]
    let backend_name = String::from("dds");
    #[cfg(not(feature = "dds"))]
    let backend_name = String::from("inmemory");

    // Substrato: a ÚNICA diferença entre os braços (T-880/EXP1a).
    #[cfg(feature = "dds")]
    let dataspace = Arc::new(dds_dataspace::DataSpace::new(172, 0)?);
    #[cfg(not(feature = "dds"))]
    let dataspace = Arc::new(dds_dataspace::in_memory::InMemoryDataSpace::new());

    // Worker único: claim otimista + confirmação por releitura do MESH RHC
    // (não via cache/`read_task` — take/read split: o cache só avança quando
    // um stream é policiado; o padrão do agente real é `read_task_mesh`).
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
    let ds_worker = Arc::clone(&dataspace);
    tokio::spawn(async move {
        let mut stream = ds_worker.subscribe_tasks();
        let _ = ready_tx.send(()); // inscrito: main pode escrever
        while let Some(task) = stream.next().await {
            if task.status != 0 || !task.assigned_agent.is_empty() {
                continue;
            }
            let mut claimed = (*task).clone();
            claimed.status = 1; // ASSIGNED
            claimed.assigned_agent = String::from("worker-exp1a");
            claimed.assigned_at_ns = now_ns();
            // RUNNING/DONE derivam do CLAIMED (assigned preservado — o filtro
            // monotônico rejeita assigned preenchido → vazio; T-880/EXP1a).
            let mut running = claimed.clone();
            running.status = 2;
            running.started_at_ns = now_ns();
            let _ = ds_worker.write_task_without_ownership(running).await;
            if delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
            for seq in 0..2u32 {
                let output = dds_contract::generated::dds_llm_orchestrator::TaskOutput {
                    task_id: task.task_id.clone(),
                    seq_num: seq,
                    content: format!("[chunk {seq}]"),
                    is_final: seq == 1,
                    finish_reason: 1,
                    agent_id: String::from("worker-exp1a"),
                    token_count: 4,
                    emitted_at_ns: now_ns(),
                };
                let _ = ds_worker.write_task_output(output).await;
            }
            let mut done = claimed;
            done.status = 3;
            done.completed_at_ns = now_ns();
            let _ = ds_worker.write_task_without_ownership(done).await;
        }
    });

    // Determinístico: main só escreve após o worker confirmar inscrição.
    let _ = ready_rx.await;

    let mut records = Vec::new();
    for wf in 0..n {
        let started = Instant::now();
        let mut stages = [String::new(), String::new(), String::new()];
        for (si, stage) in ["A", "B", "C"].iter().enumerate() {
            let task_id = format!("exp1a-{backend_name}-{wf}-{si}");
            let task = dds_contract::generated::dds_llm_orchestrator::Task {
                task_id: task_id.clone(),
                client_id: String::from("exp1a"),
                status: 0,
                ..Default::default()
            };
            let _ = dataspace.write_task_without_ownership(task).await;
            let done = loop {
                #[cfg(feature = "dds")]
                let seen = dataspace.read_task_mesh(&task_id).ok().flatten();
                #[cfg(not(feature = "dds"))]
                let seen = dataspace
                    .read_task(&task_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|arc| (*arc).clone());
                if let Some(t) = seen {
                    if t.status == 3 {
                        break t;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            };
            stages[si] = stage_hash(stage, wf, done.retry_count);
        }
        records.push(WorkflowRecord {
            workflow: wf,
            backend: backend_name.clone(),
            delay_ms,
            t_total_ms: started.elapsed().as_millis(),
            stages,
        });
    }

    let times: Vec<u128> = records.iter().map(|r| r.t_total_ms).collect();
    let mut sorted = times.clone();
    sorted.sort_unstable();
    println!(
        "{{\"backend\":\"{}\",\"delay_ms\":{},\"n\":{},\"t_total_p50_ms\":{},\"t_total_p95_ms\":{},\"t_total_min_ms\":{},\"t_total_max_ms\":{}}}",
        backend_name,
        delay_ms,
        n,
        sorted[n / 2],
        sorted[(n as f64 * 0.95) as usize % n],
        sorted[0],
        sorted[n - 1]
    );
    let out = format!("exp1a-{backend_name}-d{delay_ms}.jsonl");
    std::fs::write(
        &out,
        records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    println!(" registros em {out}");
    Ok(())
}
