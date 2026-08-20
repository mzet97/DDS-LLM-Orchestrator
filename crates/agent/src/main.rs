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

    /// Restrição de provedor publicada em `LLM.InferenceRequest`.
    #[arg(long, value_enum, default_value = "local-only")]
    provider_constraint: ProviderConstraint,
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
    use std::path::PathBuf;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();
    let spec = parse_specialization(&args.specialization);

    let security = if args.dds_secure {
        let dir = args
            .dds_security_dir
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| {
                anyhow::anyhow!("--dds-security-dir is required when --dds-secure is set")
            })?;
        #[cfg(feature = "security")]
        {
            Some(security_config_from_dir(&dir)?)
        }
        #[cfg(not(feature = "security"))]
        {
            anyhow::bail!("--dds-secure requires the security feature to be enabled at build time")
        }
    } else {
        tracing::warn!(
            "DDS running in local-only mode without authentication or encryption; \
             do not expose this deployment to untrusted networks"
        );
        None
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
    let runtime = Arc::new(AgentDds::new_with_security(config, security)?);
    #[cfg(not(feature = "security"))]
    let runtime = Arc::new(AgentDds::new(config)?);
    let _heartbeat = runtime.spawn_heartbeat();

    if args.engine == "mock" {
        let engine = Arc::new(MockEngine::new("chunk", 5, 50));
        runtime.run(engine).await?;
    } else if args.engine == "http" {
        use agent::engine_http::HttpEngine;
        if args.provider_constraint == ProviderConstraint::CloudOnly {
            anyhow::bail!("engine http é local-only; use o LLM gateway para cloud");
        }
        let engine = Arc::new(HttpEngine::new(&args.llama_url)?);
        runtime.run(engine).await?;
    } else {
        let engine = Arc::new(DdsEngine::new_with_constraint(
            args.dds_domain,
            args.agent_id,
            args.provider_constraint,
        )?);
        runtime.run(engine).await?;
    }

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
    };

    tracing::info!(agent_id = %config.agent_id, "agent SEM feature dds — caminho mock");

    let agent = Agent::new(config);
    let engine = MockEngine::new("chunk", 5, 100);
    let mut task = dds_contract::generated::dds_llm_orchestrator::Task::default();
    task.task_id = "mock-task-001".into();
    task.status = 1;
    task.messages_json = r#"[{"role":"user","content":"Hello"}]"#.into();
    agent.process_task(&task, &engine).await?;

    Ok(())
}
