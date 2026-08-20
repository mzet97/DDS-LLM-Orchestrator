#[cfg(all(feature = "dds", not(test)))]
mod app {
    use clap::Parser;
    use orchestrator::{
        dds::OrchestratorDds,
        http::{self, HttpBackend},
        http_config::{HttpConfig, HttpLimits},
    };
    use std::{
        collections::BTreeSet,
        net::{IpAddr, Ipv4Addr},
        path::PathBuf,
        sync::Arc,
        time::Duration,
    };

    #[derive(Parser)]
    #[command(name = "orchestrator", about = "DDS-first LLM orchestrator")]
    struct Args {
        #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
        bind: IpAddr,
        #[arg(long, default_value_t = 8080)]
        port: u16,
        #[arg(long)]
        http_expose: bool,
        #[arg(long)]
        http_auth_file: Option<PathBuf>,
        #[arg(long = "http-model")]
        http_models: Vec<String>,
        #[arg(long, default_value_t = 1_048_576)]
        http_body_bytes: usize,
        #[arg(long, default_value_t = 64)]
        http_message_count: usize,
        #[arg(long, default_value_t = 262_144)]
        http_message_bytes: usize,
        #[arg(long, default_value_t = 8_192)]
        http_max_tokens: u32,
        #[arg(long, default_value_t = 32)]
        http_concurrent_requests: usize,
        #[arg(long, default_value_t = 120_000)]
        http_dds_wait_timeout_ms: u64,
        #[arg(long, default_value_t = 0)]
        dds_domain: u32,
        #[arg(long)]
        dds_secure: bool,
        #[arg(long)]
        dds_security_dir: Option<PathBuf>,
        #[arg(long, default_value = "nfcm")]
        qos_manager: String,
        #[arg(long)]
        qos_profile: Option<String>,
        #[arg(long)]
        fuzzy_routing: bool,
    }

    #[cfg(feature = "security")]
    fn security_config_from_dir(
        dir: &std::path::Path,
    ) -> anyhow::Result<dds_dataspace::SecurityConfig> {
        let p = |name: &str| dir.join(name).to_string_lossy().into_owned();
        Ok(dds_dataspace::SecurityConfig::new()
            .identity_ca(p("identity_ca_cert.pem"))
            .identity_certificate(p("participant_cert.pem"))
            .identity_private_key(p("participant_key.pem"))
            .governance(p("governance.xml"))
            .permissions(p("permissions.xml"))
            .permissions_ca(p("permissions_ca_cert.pem")))
    }

    pub async fn run() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();

        let args = Args::parse();

        let security = if args.dds_secure {
            let dir = args.dds_security_dir.as_deref().ok_or_else(|| {
                anyhow::anyhow!("--dds-security-dir is required when --dds-secure is set")
            })?;
            #[cfg(feature = "security")]
            {
                Some(security_config_from_dir(dir)?)
            }
            #[cfg(not(feature = "security"))]
            {
                anyhow::bail!(
                    "--dds-secure requires the security feature to be enabled at build time"
                )
            }
        } else {
            tracing::warn!(
                "DDS running in local-only mode without authentication or encryption; \
                 do not expose this deployment to untrusted networks"
            );
            None
        };

        let http_config = HttpConfig::load(
            args.bind,
            args.port,
            args.http_expose,
            args.http_auth_file.as_deref(),
            args.http_models.into_iter().collect::<BTreeSet<_>>(),
            HttpLimits {
                body_bytes: args.http_body_bytes,
                message_count: args.http_message_count,
                message_bytes: args.http_message_bytes,
                max_tokens: args.http_max_tokens,
                concurrent_requests: args.http_concurrent_requests,
                dds_wait_timeout: Duration::from_millis(args.http_dds_wait_timeout_ms),
            },
        )?;

        let decider: Arc<dyn qos_nfcm::decider::QosDecider> = match args.qos_manager.as_str() {
            "static" => Arc::new(qos_nfcm::decider::StaticDecider::new(
                qos_nfcm::QoSProfile::Balanced,
            )),
            "zadeh" => Arc::new(qos_nfcm::zadeh::ZadehDecider::new()),
            "fcm" => Arc::new(qos_nfcm::fcm::FcmDecider::new()),
            "fcm-dhl" => Arc::new(qos_nfcm::fcm::FcmDhlDecider::default()),
            _ => Arc::new(qos_nfcm::Nfcm::qos_default()),
        };

        tracing::info!(
            qos_manager = %args.qos_manager,
            qos_profile = ?args.qos_profile,
            "QoS decider selected"
        );
        #[cfg(feature = "security")]
        let orchestrator = Arc::new(
            OrchestratorDds::new_with_security(
                args.dds_domain,
                decider,
                args.qos_profile.as_deref(),
                security,
            )?
            .with_fuzzy_routing(args.fuzzy_routing),
        );
        #[cfg(not(feature = "security"))]
        let orchestrator = Arc::new(
            OrchestratorDds::new(args.dds_domain, decider, args.qos_profile.as_deref())?
                .with_fuzzy_routing(args.fuzzy_routing),
        );
        let background = vec![
            orchestrator.spawn_cache_feeders(),
            orchestrator.spawn_registry_monitor(Duration::from_secs(15), Duration::from_secs(2)),
            orchestrator.spawn_control_loop(Duration::from_secs(2)),
            orchestrator.spawn_qos_monitor(Duration::from_secs(5)),
        ];
        let backend: Arc<dyn HttpBackend> = orchestrator;
        let app = http::router(http_config.clone(), backend);
        let address = http_config.socket_addr();
        let listener = tokio::net::TcpListener::bind(address).await?;
        tracing::info!(%address, domain = args.dds_domain, "orchestrator listening");

        let serve_result = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await;
        for task in background {
            task.abort();
            match task.await {
                Ok(()) => {}
                Err(error) if error.is_cancelled() => {}
                Err(error) => tracing::warn!(%error, "background task stopped unexpectedly"),
            }
        }
        serve_result?;
        Ok(())
    }

    async fn shutdown_signal() {
        #[cfg(unix)]
        {
            let mut terminate =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(signal) => Some(signal),
                    Err(error) => {
                        tracing::warn!(%error, "SIGTERM handler unavailable");
                        None
                    }
                };
            tokio::select! {
                result = tokio::signal::ctrl_c() => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "Ctrl-C handler unavailable");
                    }
                }
                _ = async {
                    match terminate.as_mut() {
                        Some(signal) => { signal.recv().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {}
            }
        }
        #[cfg(not(unix))]
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "Ctrl-C handler unavailable");
        }
        tracing::info!("shutdown requested");
    }
}

#[cfg(all(feature = "dds", not(test)))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}

#[cfg(not(feature = "dds"))]
fn main() {
    eprintln!("orchestrator: build without `dds` feature; use --features dds");
}
