//! # dds-bench — CLI de benchmark
//!
//! Roda um cenário E1–E5/OP1–OP4 contra a malha DDS e grava JSONL.
//!
//! ## Uso
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p benchmarks --features dds -- \
//!   --scenario E4 --domain 0 --duration 60 --arm nfcm --out ./bench_out
//! ```

#[cfg(feature = "dds")]
mod app {
    use anyhow::{bail, Result};
    use benchmarks::{BenchmarkDriver, DriverConfig};
    use std::path::PathBuf;

    #[tokio::main]
    pub async fn main() -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();

        let mut scenario_id = "E4".to_string();
        let mut cfg = DriverConfig::default();
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--scenario" => {
                    scenario_id = args.get(i + 1).cloned().unwrap_or_else(|| "E4".into());
                    i += 2;
                }
                "--domain" => {
                    cfg.domain = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    i += 2;
                }
                "--duration" => {
                    cfg.duration_s = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    i += 2;
                }
                "--seed" => {
                    cfg.seed = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(42);
                    i += 2;
                }
                "--out" => {
                    if let Some(v) = args.get(i + 1) {
                        cfg.out_dir = PathBuf::from(v);
                    }
                    i += 2;
                }
                "--model" => {
                    if let Some(v) = args.get(i + 1) {
                        cfg.model_name = v.clone();
                    }
                    i += 2;
                }
                "--arm" => {
                    if let Some(v) = args.get(i + 1) {
                        cfg.qos_arm = v.clone();
                    }
                    i += 2;
                }
                "--workers" => {
                    cfg.workers = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(10);
                    i += 2;
                }
                "--timeout-ms" => {
                    cfg.timeout_ms = args
                        .get(i + 1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(30_000);
                    i += 2;
                }
                "--list" => {
                    for s in benchmarks::registry() {
                        println!("{:<4} {:<32} fonte: {}", s.id, s.name, s.source);
                    }
                    return Ok(());
                }
                other => bail!("flag desconhecida: {other} (use --list para cenários)"),
            }
        }

        let scenario = benchmarks::get_scenario(&scenario_id)
            .ok_or_else(|| anyhow::anyhow!("cenário desconhecido: {scenario_id}"))?
            .clone();

        tracing::info!(
            scenario = %scenario.id,
            name = %scenario.name,
            domain = cfg.domain,
            duration_s = cfg.duration_s,
            arm = %cfg.qos_arm,
            "dds-bench iniciando"
        );

        let driver = BenchmarkDriver::new(cfg, scenario)?;
        let summary = driver.run().await?;

        println!(
            "submetidas={} ok={} erros={} timeouts={} em {:.1}s → {}",
            summary.submitted,
            summary.ok,
            summary.errors,
            summary.timeouts,
            summary.elapsed_s,
            summary.out_file.display()
        );
        Ok(())
    }
}

#[cfg(feature = "dds")]
fn main() -> anyhow::Result<()> {
    app::main()
}

#[cfg(not(feature = "dds"))]
fn main() {
    eprintln!("dds-bench: build sem feature `dds` — nada a fazer (use --features dds)");
    eprintln!("cenários disponíveis no código: E1..E5, OP1..OP4");
}
