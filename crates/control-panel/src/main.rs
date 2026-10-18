//! Binário `control-panel` (paridade com `control_panel.cli` do Python):
//! `supervisor`/`panel` sobem o servidor; `status`, `stop-all` e `catalog`
//! falam com o supervisor em execução.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use anyhow::{Context, Result};
use control_panel::config::PanelConfig;
use control_panel::server::{router, AppState};
use control_panel::supervisor::Supervisor;

fn http_request(cfg: &PanelConfig, method: &str, path: &str) -> Result<String> {
    let addr = format!("{}:{}", cfg.supervisor_host, cfg.supervisor_port);
    let mut stream =
        TcpStream::connect(&addr).with_context(|| format!("supervisor indisponivel em {addr}"))?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nhost: {addr}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw)?;
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    Ok(body)
}

async fn serve(cfg: PanelConfig) -> Result<()> {
    std::fs::create_dir_all(&cfg.log_dir).ok();
    let supervisor = Arc::new(Supervisor::new(cfg.clone()));
    let state = Arc::new(AppState::new(supervisor.clone()));
    let addr = format!("{}:{}", cfg.supervisor_host, cfg.supervisor_port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    println!("control-panel supervisor em http://{addr}");
    let shutdown = {
        let supervisor = supervisor.clone();
        async move {
            let _ = tokio::signal::ctrl_c().await;
            supervisor.stop_all();
        }
    };
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await
        .context("servidor HTTP")?;
    supervisor.stop_all();
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = PanelConfig::from_env();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("status") => {
            let body = http_request(&cfg, "GET", "/status")?;
            let services: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap_or_default();
            for svc in services {
                let status = if svc["running"].as_bool().unwrap_or(false) {
                    "rodando"
                } else {
                    "parado"
                };
                println!(
                    "{:<30} {:<10} pid={}",
                    svc["name"].as_str().unwrap_or("?"),
                    status,
                    svc["pid"]
                );
            }
            Ok(())
        }
        Some("stop-all") => {
            println!("{}", http_request(&cfg, "POST", "/services/stop-all")?);
            Ok(())
        }
        Some("catalog") => {
            println!("{}", http_request(&cfg, "GET", "/catalog")?);
            Ok(())
        }
        // `supervisor` e `panel` são sinônimos: um processo só (supervisor+web).
        Some("supervisor") | Some("panel") | None => serve(cfg).await,
        Some(other) => {
            eprintln!(
                "comando desconhecido: {other} (use supervisor|panel|status|stop-all|catalog)"
            );
            std::process::exit(2);
        }
    }
}
