//! Sonda de diagnóstico (fase 880): `stream_tasks` entrega o write de
//! `write_task_without_ownership` feito no MESMO participante?
//! `cargo run --release -p dds-dataspace --features dds --example stream_probe -- <domínio>

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let domain: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(175);
    let dataspace = Arc::new(dds_dataspace::DataSpace::new(domain, 0)?);
    use dds_dataspace::api::DataSpaceApi;

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
    let ds_worker = std::sync::Arc::clone(&dataspace);
    let worker = tokio::spawn(async move {
        use futures::StreamExt;
        let mut stream = ds_worker.subscribe_tasks();
        let _ = ready_tx.send(());
        match tokio::time::timeout(std::time::Duration::from_secs(8), stream.next()).await {
            Ok(Some(task)) => {
                eprintln!(
                    "[probe-worker] RECEBIDO {} status={}",
                    task.task_id, task.status
                );
                // Réplica do worker do exp1a: write DENTRO do processamento
                // do stream + confirmação por read_task em loop.
                eprintln!("[probe-worker] claimando...");
                let mut claimed = (*task).clone();
                claimed.status = 1;
                claimed.assigned_agent = String::from("probe");
                match ds_worker.write_task_without_ownership(claimed).await {
                    Ok(()) => eprintln!("[probe-worker] ASSIGNED escrito"),
                    Err(e) => eprintln!("[probe-worker] ERRO no write: {e}"),
                }
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
                loop {
                    if tokio::time::Instant::now() > deadline {
                        eprintln!("[probe-worker] confirm: TIMEOUT sem ver ASSIGNED próprio");
                        break;
                    }
                    if let Ok(Some(t)) = ds_worker.read_task(&task.task_id).await {
                        eprintln!(
                            "[probe-worker] confirm lê status={} assigned={:?}",
                            t.status, t.assigned_agent
                        );
                        if t.status == 1 && t.assigned_agent == "probe" {
                            eprintln!("[probe-worker] claim CONFIRMADO");
                            break;
                        }
                    } else {
                        eprintln!("[probe-worker] confirm: read_task None");
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
            Ok(None) => eprintln!("[probe-worker] stream encerrado"),
            Err(_) => eprintln!("[probe-worker] TIMEOUT 8s sem amostra"),
        }
    });

    let _ = ready_rx.await;
    // Rede de segurança: espera o waitset assentar o registro do reader.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let task = dds_contract::generated::dds_llm_orchestrator::Task {
        task_id: String::from("probe-1"),
        status: 0,
        ..Default::default()
    };
    eprintln!("[probe-main] escrevendo probe-1");
    dataspace
        .write_task_without_ownership(task)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    eprintln!("[probe-main] escrito");
    let _ = worker.await;
    Ok(())
}
