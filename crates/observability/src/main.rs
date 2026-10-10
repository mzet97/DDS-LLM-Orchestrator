//! # Observability Collector — Entry Point
//!
//! Serviço de observabilidade que coleta QoS metrics, violations, discovery events
//! e execution traces via DDS, persistindo em sink JSONL.
//!
//! ## Uso
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p observability --features dds -- --dds-domain 0
//! ```

#[cfg(feature = "dds")]
mod app {
    use anyhow::Result;
    use observability::{FileEventSink, QosCollector, TraceCollector};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::main]
    pub async fn main() -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();

        let mut domain: u32 = 0;
        let mut output_dir = "./observability_output".to_string();
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--dds-domain" => {
                    domain = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    i += 2;
                }
                "--output-dir" => {
                    if let Some(v) = args.get(i + 1) {
                        output_dir = v.clone();
                    }
                    i += 2;
                }
                _ => i += 1,
            }
        }

        tracing::info!(domain, output_dir = %output_dir, "observability collector iniciando");

        // Inicializa componentes
        let sink: Arc<dyn observability::EventSink> =
            Arc::new(FileEventSink::new(format!("{}/events.jsonl", output_dir))?);
        let qos_store = Arc::new(observability::QosStore::new());
        let trace_collector = Arc::new(TraceCollector::new(&output_dir)?);

        // Cria DDS DataSpace
        let dataspace = Arc::new(dds_dataspace::DataSpace::new(domain, 200)?); // orchestrator strength

        // Sobe os loops de ingestão (QoS + Execution.Trace)
        let qos = Arc::new(QosCollector::new(Arc::clone(&qos_store), Arc::clone(&sink)));
        let _handles = observability::dds::spawn_ingestion(
            Arc::clone(&dataspace),
            Arc::clone(&qos),
            Arc::clone(&trace_collector),
            Duration::from_secs(5),
        );

        tracing::info!("coletores iniciados, aguardando eventos...");

        // Loop principal: flush periódico + SIGINT gracioso (REQ/T-820-04) —
        // o Ctrl+C faz flush do sink e do trace_collector antes de sair, sem
        // perder o buffer pendente.
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    sink.flush().ok();
                    trace_collector.flush().ok();
                    let stats = qos.stats();
                    tracing::info!(
                        metrics = stats.total_metrics,
                        violations = stats.total_violations,
                        discoveries = stats.total_discoveries,
                        "observability snapshot"
                    );
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT recebido — flush final antes de sair");
                    break;
                }
            }
        }

        // Flush final (melhor esforço; erros logados, não silenciados).
        if let Err(e) = sink.flush() {
            tracing::warn!(error = %e, "flush final do sink falhou");
        }
        if let Err(e) = trace_collector.flush() {
            tracing::warn!(error = %e, "flush final do trace_collector falhou");
        }
        let stats = qos.stats();
        tracing::info!(
            metrics = stats.total_metrics,
            violations = stats.total_violations,
            discoveries = stats.total_discoveries,
            traces = trace_collector.event_count(),
            "observability collector encerrado"
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
    eprintln!("observability: build sem feature `dds` — nada a fazer (use --features dds)");
}
