//! EXP1a (dissertação §3.7.3): ablação de substrato no MESMO binário —
//! `InMemoryDataSpace` vs `DataSpace` (CycloneDDS real), sobre a mesma
//! trait [`dds_dataspace::api::DataSpaceApi`], com o MESMO worker
//! (claim → ASSIGNED → 2 chunks → DONE) e o mesmo fluxo A→B→C.
//!
//! Build/run:
//!   cargo run --release -p dds-dataspace --example exp1a -- --n 30 --delay-ms 0
//!   cargo run --release -p dds-dataspace --features dds --example exp1a -- --n 30 --delay-ms 50
//!
//! A variável independente é o substrato de comunicação; runtime, worker e
//! fluxo permanecem fixos (T-880-EXP1a).

use std::time::Instant;

#[derive(Clone, serde::Serialize)]
struct WorkflowRecord {
    workflow: usize,
    backend: String,
    delay_ms: u64,
    t_total_ms: u128,
    stages: [String; 3],
}

fn stage_hash(stage: &str, wf: usize, attempt: u32) -> String {
    // Determinístico e dependente do conteúdo (análogo ao fixture):
    let mut h: u64 = 1469598103934665603;
    for b in format!("{stage}:{wf}:{attempt}").as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut n = 30usize;
    let mut delay_ms: u64 = 0;
    let mut args = std::env::args().skip(1);
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

    // Substrato: a ÚNICA diferença entre os braços (T-880-EXP1a).
    #[cfg(feature = "dds")]
    let dataspace = std::sync::Arc::new(dds_dataspace::DataSpace::new(171, 0)?);
    #[cfg(not(feature = "dds"))]
    let dataspace = std::sync::Arc::new(dds_dataspace::in_memory::InMemoryDataSpace::new());

    use dds_dataspace::api::DataSpaceApi;

    // Worker único: claim otimista + confirmação por leitura + execução.
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn detached de propósito: o worker vive até o fim do `main` —
    // abortar pelo JoinHandle encerraria o worker antes do fim do fluxo.
    {
        let ds = std::sync::Arc::clone(&dataspace);
        let delay = delay_ms;
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut stream = Some(ds.subscribe_tasks());
            let _ = ready_tx.send(()); // inscrito: main pode escrever
            while let Some(task) = stream.as_mut().unwrap().next().await {
                if task.status != 0 || !task.assigned_agent.is_empty() {
                    continue;
                }
                let mut claimed = (*task).clone();
                claimed.status = 1; // ASSIGNED
                claimed.assigned_agent = String::from("worker-exp1a");
                claimed.assigned_at_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                eprintln!("[worker] claimando {}", task.task_id);
                match ds.write_task_without_ownership(claimed.clone()).await {
                    Ok(()) => eprintln!("[worker] ASSIGNED escrito"),
                    Err(e) => eprintln!("[worker] ERRO no write ASSIGNED: {e}"),
                }
                // Confirmação por releitura do MESH RHC (não-consumidora e
                // independente de polling de stream) — o padrão do agente
                // real (`read_task_mesh` + `confirm_ownership`). A stream de
                // Tasks tem filtro de conteúdo (só entrega o que interessa ao
                // claim) e NÃO devolve o ASSIGNED — T-880/EXP1a.
                let confirmed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    loop {
                        #[cfg(feature = "dds")]
                        let seen = ds.read_task_mesh(&task.task_id).ok().flatten();
                        #[cfg(not(feature = "dds"))]
                        let seen = ds
                            .read_task(&task.task_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|arc| (*arc).clone());
                        if let Some(t) = seen {
                            if t.status == 1 && t.assigned_agent == "worker-exp1a" {
                                return true;
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap_or(false);
                if !confirmed {
                    continue;
                }
                // Padrão do agente real: RUNNING/DONE derivam do CLAIMED
                // (assigned_agent preservado — o filtro monotônico rejeita
                // assigned preenchido -> vazio).
                let mut running = claimed.clone();
                running.status = 2; // RUNNING
                let _ = ds.write_task_without_ownership(running).await;
                if delay > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
                for seq in 0..2u64 {
                    let output = dds_contract::generated::dds_llm_orchestrator::TaskOutput {
                        task_id: task.task_id.clone(),
                        seq_num: seq as u32,
                        content: format!("[chunk {seq}]"),
                        is_final: seq == 1,
                        finish_reason: 1,
                        agent_id: String::from("worker-exp1a"),
                        token_count: 4,
                        emitted_at_ns: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos() as u64,
                    };
                    let _ = ds.write_task_output(output).await;
                }
                let mut done = claimed.clone();
                done.status = 3; // DONE
                done.completed_at_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                let _ = ds.write_task_without_ownership(done).await;
            }
        });
    }

    // Fluxo A→B→C: 3 estágios sequenciais, cada um uma task no substrato.
    // Determinístico: worker inscreve primeiro; main inscreve a sua stream
    // e só então escreve (no mock não há TransientLocal para repor amostra).
    let _ = ready_rx.await;
    eprintln!("[main] pronto; escrevendo");
    // No braço DDS, main e worker escrevem pelo MESMO writer (cliente): sob
    // Exclusive Ownership, writers diferentes em empate de strength podem
    // perder amostras no RHC — e a variável do EXP1a é o substrato, não a
    // arbitragem. A stream já alimenta o cache em amostras aceitas.
    let mut records = Vec::new();
    for wf in 0..n {
        let started = Instant::now();
        let mut stages = [String::new(), String::new(), String::new()];
        for (si, stage) in ["A", "B", "C"].iter().enumerate() {
            let task_id = format!("exp1a-{backend_name}-{wf}-{si}");
            let mut task = dds_contract::generated::dds_llm_orchestrator::Task {
                task_id: task_id.clone(),
                client_id: String::from("exp1a"),
                ..Default::default()
            };
            task.status = 0;
            eprintln!("[main] wf{} stage{} escrevendo", wf, si);
            let _ = dataspace.write_task_without_ownership(task).await;
            // Aguarda DONE via releitura do MESH/cache (não via stream —
            // mesmo take/read split do confirm do worker).
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

    // Worker detached (spawn acima): JoinHandle descartado de propósito —
    // abortar aqui encerraria o worker antes do fim do fluxo.
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
    let out = format!("exp1a-{}-d{}.jsonl", backend_name, delay_ms);
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
