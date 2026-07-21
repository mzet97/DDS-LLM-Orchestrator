//! Binário `context-store` — entry point do serviço (port de
//! `context_store/main.py`).
//!
//! ```text
//! context-store [--dds-domain N] [--data-file PATH] [--log-level LEVEL]
//! ```
//!
//! - `--dds-domain`: domínio DDS (default 0, como o Python);
//! - `--data-file`: journal JSONL (default `context_store.jsonl`);
//! - `--log-level`: filtro de log (default INFO; respeita `RUST_LOG` se setado).
//!
//! Encerra graciosamente em SIGINT/SIGTERM (paridade com os signal handlers
//! do `main.py`).

use anyhow::{Context, Result};
use context_store::{ContextStoreService, LocalContextStore};
use std::sync::Arc;

struct Args {
    dds_domain: u32,
    data_file: String,
    log_level: String,
}

fn parse_args() -> Result<Args> {
    let mut args = Args {
        dds_domain: 0,
        data_file: "context_store.jsonl".to_string(),
        log_level: "INFO".to_string(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dds-domain" => {
                let v = it.next().context("--dds-domain requer um valor")?;
                args.dds_domain = v.parse().context("--dds-domain deve ser inteiro")?;
            }
            "--data-file" => {
                args.data_file = it.next().context("--data-file requer um valor")?;
            }
            "--log-level" => {
                args.log_level = it.next().context("--log-level requer um valor")?;
            }
            "-h" | "--help" => {
                println!(
                    "Context Store — DDS ↔ journal local\n\
                     Uso: context-store [--dds-domain N] [--data-file PATH] [--log-level LEVEL]"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("argumento desconhecido: {other} (use --help)"),
        }
    }
    Ok(args)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = parse_args()?;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(args.log_level.to_lowercase()));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let store = LocalContextStore::open(&args.data_file)
        .await
        .with_context(|| format!("abrindo journal {}", args.data_file))?;
    if let Some(path) = store.journal_path() {
        tracing::info!(path = %path.display(), "journal ativo");
    }
    let service =
        ContextStoreService::new(args.dds_domain, Arc::new(store)).context("subindo DataSpace")?;

    // SIGINT/SIGTERM → shutdown gracioso (paridade com main.py).
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            r = tokio::signal::ctrl_c() => {
                if let Err(e) = r {
                    tracing::warn!(error = %e, "falha no handler de Ctrl+C");
                }
            }
            _ = async {
                match &mut sigterm {
                    Some(s) => {
                        s.recv().await;
                    }
                    None => std::future::pending::<()>().await,
                }
            } => {}
        }
        let _ = stop_tx.send(());
    });

    service
        .run(async move {
            let _ = stop_rx.await;
        })
        .await;
    Ok(())
}
