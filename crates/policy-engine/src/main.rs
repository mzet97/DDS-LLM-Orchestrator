//! # policy-engine — fonte da verdade das políticas de segurança
//!
//! Substitui `src/orchestrator/policy_engine/main.py` (Python): lê
//! `policies.json`, publica `Security.PolicySnapshot` no DDS e re-publica
//! periodicamente para late-joiners.
//!
//! Uso:
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p policy-engine --features dds -- \
//!     --dds-domain 0 --policy-file crates/policy-engine/policies.json
//! ```

#[cfg(feature = "dds")]
mod app {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use policy_engine::service::{PolicyEngineService, DEFAULT_REPUBLISH_INTERVAL};

    pub async fn run() -> anyhow::Result<()> {
        let mut domain: u32 = 0;
        let mut policy_file = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/policies.json"));
        let mut republish = DEFAULT_REPUBLISH_INTERVAL;
        let mut log_level = "info".to_string();

        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--dds-domain" => {
                    domain = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    i += 2;
                }
                "--policy-file" => {
                    if let Some(v) = args.get(i + 1) {
                        policy_file = PathBuf::from(v);
                    }
                    i += 2;
                }
                "--republish-interval-secs" => {
                    let secs = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(60);
                    republish = Duration::from_secs(secs);
                    i += 2;
                }
                "--log-level" => {
                    if let Some(v) = args.get(i + 1) {
                        log_level = v.clone();
                    }
                    i += 2;
                }
                _ => i += 1,
            }
        }

        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
            )
            .init();

        let data_space =
            dds_dataspace::DataSpace::new(domain, dds_dataspace::DataSpace::STRENGTH_ORCHESTRATOR)
                .map_err(|e| {
                    anyhow::anyhow!("falha ao subir DataSpace no domínio {domain}: {e}")
                })?;

        let service = PolicyEngineService::new(Arc::new(data_space), policy_file)
            .with_republish_interval(republish);

        tokio::select! {
            res = service.run() => {
                res?;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("SIGINT — encerrando policy-engine");
            }
        }
        Ok(())
    }
}

#[cfg(feature = "dds")]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    app::run().await
}

#[cfg(not(feature = "dds"))]
fn main() {
    eprintln!("policy-engine: build sem feature `dds` — nada a fazer (use --features dds)");
}
