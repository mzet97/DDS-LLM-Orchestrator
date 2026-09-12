//! `submit-one`: submete UMA task via DDS e imprime o resultado como JSON.
//!
//! Interface para harnesses externos (ex. benchmark Python) dirigirem cadeias:
//! cada etapa é uma task real reivindicada via DDS e inferida no backend real.
//!
//! Uso: `submit-one -- <domain> <model> <messages_json> [--timeout-ms N]`
//! Saída (stdout): `{"task_id","content","success","latency_ms"}`.
//! Requer `--features dds` (usa `DdsClientDds`).

use client::dds_impl::DdsClientDds;
use client::{ClientConfig, DdsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("uso: submit-one -- <domain> <model> <messages_json> [--timeout-ms N]");
        std::process::exit(2);
    }
    let domain: u32 = args[1].parse()?;
    let model = args[2].clone();
    let messages_json = args[3].clone();
    let mut timeout_ms = 120_000u64;
    let mut i = 4;
    while i < args.len() {
        if args[i] == "--timeout-ms" {
            timeout_ms = args.get(i + 1).ok_or("--timeout-ms exige valor")?.parse()?;
            i += 2;
        } else {
            return Err(format!("argumento desconhecido: {}", args[i]).into());
        }
    }

    let config = ClientConfig {
        client_id: "submit-one".to_string(),
        dds_domain: domain,
        timeout_ms,
    };
    let helper = DdsClient::new(config.clone());
    let client = DdsClientDds::new(config)?;
    let task = helper.create_task(&model, &messages_json, 5, false);
    let task_id = task.task_id.clone();
    match client.submit(task).await {
        Ok(r) => {
            println!(
                "{}",
                serde_json::json!({
                    "task_id": r.task_id,
                    "content": r.content,
                    "success": r.success,
                    "latency_ms": r.latency_ms,
                    "tokens_prompt": r.tokens_prompt,
                    "tokens_completion": r.tokens_completion,
                })
            );
            if !r.success {
                eprintln!("submit-one: task {task_id} sem sucesso");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("submit-one: task {task_id} falhou: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}
