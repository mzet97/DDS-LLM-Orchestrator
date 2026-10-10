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
    use std::str::FromStr;

    /// Parse estrito de flag numérica (REQ/T-820-13): valor ausente ou
    /// inválido é ERRO — fallback silencioso invalida a comparação entre
    /// braços (ex.: `--duration abc` rodaria com a duração do cenário).
    fn parse_flag<T: FromStr>(args: &[String], i: usize, flag: &str) -> Result<T> {
        let raw = args
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("{flag} requer um valor"))?;
        raw.parse::<T>()
            .map_err(|_| anyhow::anyhow!("valor inválido para {flag}: '{raw}'"))
    }

    #[tokio::main]
    pub async fn main() -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();

        let mut scenario_id: Option<String> = None;
        let mut cfg = DriverConfig::default();
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--scenario" => {
                    scenario_id = Some(
                        args.get(i + 1)
                            .ok_or_else(|| anyhow::anyhow!("--scenario requer um valor"))?
                            .clone(),
                    );
                    i += 2;
                }
                "--domain" => {
                    cfg.domain = parse_flag(&args, i, "--domain")?;
                    i += 2;
                }
                "--duration" => {
                    cfg.duration_s = parse_flag(&args, i, "--duration")?;
                    i += 2;
                }
                "--seed" => {
                    cfg.seed = parse_flag(&args, i, "--seed")?;
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
                    cfg.workers = parse_flag(&args, i, "--workers")?;
                    i += 2;
                }
                "--timeout-ms" => {
                    cfg.timeout_ms = parse_flag(&args, i, "--timeout-ms")?;
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

        let scenario_id = scenario_id.unwrap_or_else(|| "E4".into());
        let scenario = benchmarks::get_scenario(&scenario_id)
            .ok_or_else(|| anyhow::anyhow!("cenário desconhecido: {scenario_id}"))?
            .clone();

        tracing::info!(
            scenario = %scenario.id,
            name = %scenario.name,
            domain = cfg.domain,
            duration_s = cfg.duration_s,
            arm = %cfg.qos_arm,
            workers = cfg.workers,
            "dds-bench iniciando"
        );

        let driver = BenchmarkDriver::new(cfg, scenario)?;
        let summary = driver.run().await?;

        let concurrency = if summary.concurrency_levels.is_empty() {
            String::new()
        } else {
            format!(" concurrency={:?}", summary.concurrency_levels)
        };
        println!(
            "submetidas={} ok={} erros={} timeouts={} em {:.1}s{} → {}",
            summary.submitted,
            summary.ok,
            summary.errors,
            summary.timeouts,
            summary.elapsed_s,
            concurrency,
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
