//! # Agent — 1º alvo da migração (maior ROI)
//!
//! Substitui `src/orchestrator/agent/` (~2,0k LOC Python): assume tasks PENDING
//! via DDS (claim com confirmação de ownership), faz a ponte com o llama-server
//! C++ e faz streaming dos chunks de volta.
//!
//! ## Uso
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p agent --features dds -- \
//!     --agent-id agent-rust-01 --slots 8 --dds-domain 0
//! # engine mock (sem llama-server):
//! CYCLONEDDS_STATIC=1 cargo run -p agent --features dds -- --engine mock
//! ```

use agent::claim::Specialization;
use agent::engine::ProviderConstraint;
#[cfg(not(feature = "dds"))]
use agent::Agent;
use agent::AgentConfig;
use anyhow::Result;
use clap::Parser;
#[cfg(feature = "dds")]
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "agent", about = "DDS-LLM Agent (Rust)")]
struct Args {
    /// Agent ID
    #[arg(long, default_value = "agent-rust-01")]
    agent_id: String,

    /// DDS Domain ID
    #[arg(long, default_value_t = 0)]
    dds_domain: u32,

    /// Enable DDS Security (requires --dds-security-dir)
    #[arg(long)]
    dds_secure: bool,

    /// Directory containing DDS Security certificates and policy files
    #[arg(long)]
    dds_security_dir: Option<String>,

    /// Number of concurrent slots
    #[arg(long, default_value_t = 8)]
    slots: u32,

    /// Model name
    #[arg(long, default_value = "qwen3.5-0.8b")]
    model: String,

    /// Specialization (text, vision, embedding, transcription)
    #[arg(long, default_value = "text")]
    specialization: String,

    /// Engine: dds (llama-server via LLM.*), http (llama-server via HTTP), ou mock (teste)
    #[arg(long, default_value = "dds")]
    engine: String,

    /// llama-server HTTP URL (for http engine)
    #[arg(long, default_value = "http://localhost:8082")]
    llama_url: String,

    /// Restrição de claim: só aceita tasks cujo `target_agent` comece com este
    /// prefixo (vazio = aceita qualquer target; EXP4 injeção forçada).
    #[arg(long, default_value = "")]
    target_agent_prefix: String,

    /// Restrição de provedor publicada em `LLM.InferenceRequest`.
    #[arg(long, value_enum, default_value = "local-only")]
    provider_constraint: ProviderConstraint,

    /// T-900-M3-02: liga o modo adaptativo (barreira Plan/State/Task para
    /// tasks `laya1-`; tasks legadas seguem as regras clássicas intocadas).
    #[arg(long, default_value_t = false)]
    adaptive: bool,

    /// Capacidades anunciadas em `Agent.Capabilities` (catálogo rev 1;
    /// obrigatórias com --adaptive).
    #[arg(long, default_value = "")]
    adaptive_capabilities: String,

    /// Domínio DDS dedicado do plano adaptativo (isolamento D-ISOLATION).
    #[arg(long, default_value_t = 0)]
    adaptive_domain: u32,

    /// O-4: período de publicação do heartbeat (AgentState em AgentRegistry).
    /// Default 5 s (comportamento histórico). O reaper do orchestrator usa
    /// --agents-stale-after-ms; piso prático = lease DDSI de ownership.
    #[arg(long, default_value_t = 5_000)]
    heartbeat_ms: u64,
}

fn parse_specialization(s: &str) -> Specialization {
    match s.to_lowercase().as_str() {
        "text" => Specialization::Text,
        "vision" => Specialization::Vision,
        "embedding" => Specialization::Embedding,
        "transcription" => Specialization::Transcription,
        _ => Specialization::Text,
    }
}

#[cfg(feature = "security")]
fn security_config_from_dir(dir: &std::path::Path) -> Result<dds_dataspace::SecurityConfig> {
    let p = |name: &str| dir.join(name).to_string_lossy().into_owned();
    Ok(dds_dataspace::SecurityConfig::new()
        .identity_ca(p("identity_ca_cert.pem"))
        .identity_certificate(p("participant_cert.pem"))
        .identity_private_key(p("participant_key.pem"))
        .governance(p("governance.xml"))
        .permissions(p("permissions.xml"))
        .permissions_ca(p("permissions_ca_cert.pem")))
}

#[cfg(feature = "dds")]
#[tokio::main]
async fn main() -> Result<()> {
    use agent::dds::AgentDds;
    use agent::engine::MockEngine;
    use agent::engine_dds::DdsEngine;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();
    let spec = parse_specialization(&args.specialization);

    if args.dds_secure {
        #[cfg(not(feature = "security"))]
        anyhow::bail!("--dds-secure requires the security feature to be enabled at build time");
    } else {
        tracing::warn!(
            "DDS running in local-only mode without authentication or encryption; \
             do not expose this deployment to untrusted networks"
        );
    }

    // T-900-M3-02: config extra para o runtime adaptativo (o base é movido).
    #[cfg(feature = "dds")]
    let adaptive_config = AgentConfig {
        agent_id: args.agent_id.clone(),
        hostname: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        model: args.model.clone(),
        specialization: spec,
        slots: args.slots,
        dds_domain: args.dds_domain,
        target_agent_prefix: args.target_agent_prefix.clone(),
    };

    let config = AgentConfig {
        agent_id: args.agent_id.clone(),
        hostname: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        model: args.model,
        specialization: spec,
        slots: args.slots,
        dds_domain: args.dds_domain,
        target_agent_prefix: args.target_agent_prefix.clone(),
    };

    tracing::info!(
        agent_id = %config.agent_id,
        domain = config.dds_domain,
        slots = config.slots,
        model = %config.model,
        specialization = ?spec,
        engine = %args.engine,
        "agent iniciando (DDS)"
    );

    #[cfg(feature = "security")]
    let runtime = {
        use std::path::PathBuf;
        let security = if args.dds_secure {
            let dir = args
                .dds_security_dir
                .as_deref()
                .map(PathBuf::from)
                .ok_or_else(|| {
                    anyhow::anyhow!("--dds-security-dir is required when --dds-secure is set")
                })?;
            Some(security_config_from_dir(&dir)?)
        } else {
            None
        };
        Arc::new(AgentDds::new_with_security(config, security)?)
    };
    #[cfg(not(feature = "security"))]
    let runtime = Arc::new(AgentDds::new(config)?);

    // T-900-M3-02: modo adaptativo OPT-IN — barreira + feeders + auto-anúncio.
    #[cfg(feature = "dds")]
    let _adaptive_guard = if args.adaptive {
        use agent::adaptive::dds_feeder;
        use agent::adaptive::AdaptiveBarrier;
        use std::sync::Arc;

        let caps: Vec<String> = args
            .adaptive_capabilities
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        anyhow::ensure!(
            !caps.is_empty(),
            "--adaptive exige --adaptive-capabilities (rótulos do catálogo rev 1)"
        );
        let barrier = Arc::new(AdaptiveBarrier::new(&args.agent_id));
        let space = Arc::new(dds_feeder::mount(args.adaptive_domain)?);
        dds_feeder::spawn_feeders(Arc::clone(&space), Arc::clone(&barrier)).await;
        let announcer = tokio::spawn(dds_feeder::announce_self(
            space,
            args.agent_id.clone(),
            caps,
            workflow_engine::catalog_v1().revision,
        ));
        // AgentDds não é Clone: recria o runtime base e aplica a barreira.
        let rt = AgentDds::new(adaptive_config.clone())?.with_adaptive(barrier);
        Some((Arc::new(rt), announcer))
    } else {
        None
    };
    // rebinding: usa o runtime COM barreira quando --adaptive
    let runtime = match _adaptive_guard.as_ref() {
        Some((rt, _)) => Arc::clone(rt),
        None => runtime,
    };
    // P2 (review PR#10): o heartbeat corre NO runtime que processa (com
    // adaptive, o runtime original nunca via os slots/completions — o
    // registry reportava o agente permanentemente idle).
    let _heartbeat = runtime.spawn_heartbeat(std::time::Duration::from_millis(args.heartbeat_ms));

    // T-820-09: valor desconhecido de `--engine` era engolido pelo `else` e
    // caía no branch DDS silenciosamente (ex.: typo `--engine ddd` subia um
    // agente DDS achando que era outro engine). Agora é erro explícito no boot.
    match args.engine.as_str() {
        "mock" => {
            let engine = Arc::new(MockEngine::new("chunk", 5, 50));
            runtime.run(engine).await?;
        }
        "http" => {
            use agent::engine_http::HttpEngine;
            if args.provider_constraint == ProviderConstraint::CloudOnly {
                anyhow::bail!("engine http é local-only; use o LLM gateway para cloud");
            }
            let engine = Arc::new(HttpEngine::new(&args.llama_url)?);
            runtime.run(engine).await?;
        }
        "dds" => {
            let engine = Arc::new(DdsEngine::new_with_constraint(
                args.dds_domain,
                args.agent_id,
                args.provider_constraint,
            )?);
            runtime.run(engine).await?;
        }
        other => anyhow::bail!("--engine inválido: '{other}' (valores válidos: dds | http | mock)"),
    }

    Ok(())
}

#[cfg(not(feature = "dds"))]
#[tokio::main]
async fn main() -> Result<()> {
    use agent::engine::MockEngine;

    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let spec = parse_specialization(&args.specialization);

    let config = AgentConfig {
        agent_id: args.agent_id.clone(),
        hostname: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        model: args.model,
        specialization: spec,
        slots: args.slots,
        dds_domain: args.dds_domain,
        target_agent_prefix: args.target_agent_prefix.clone(),
    };

    tracing::info!(agent_id = %config.agent_id, "agent SEM feature dds — caminho mock");

    let agent = Agent::new(config);
    let engine = MockEngine::new("chunk", 5, 100);
    let task = dds_contract::generated::dds_llm_orchestrator::Task {
        task_id: "mock-task-001".into(),
        status: 1,
        messages_json: r#"[{"role":"user","content":"Hello"}]"#.into(),
        ..Default::default()
    };
    agent.process_task(&task, &engine).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_defaults_to_local_only_and_accepts_cloud_only() {
        let defaults = Args::try_parse_from(["agent"]).unwrap();
        assert_eq!(defaults.provider_constraint, ProviderConstraint::LocalOnly);

        let explicit =
            Args::try_parse_from(["agent", "--provider-constraint", "cloud-only"]).unwrap();
        assert_eq!(explicit.provider_constraint, ProviderConstraint::CloudOnly);
    }
}
