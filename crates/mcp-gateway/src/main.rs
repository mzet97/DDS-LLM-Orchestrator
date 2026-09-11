//! Binário `mcp-gateway` (feature `dds`): sobe o serviço no domínio e aguarda.
//!
//! Equivalente ao `main.py` Python:
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p mcp-gateway --features dds -- \
//!     --dds-domain 0 --filesystem-root /tmp/sandbox
//! ```

use anyhow::{Context, Result};
use std::path::PathBuf;

struct Args {
    dds_domain: u32,
    dds_secure: bool,
    dds_security_dir: Option<String>,
    filesystem_root: String,
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

fn parse_args() -> Result<Args> {
    let mut dds_domain = 0u32;
    let mut dds_secure = false;
    let mut dds_security_dir: Option<String> = None;
    let mut filesystem_root = "/tmp/sandbox".to_string();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dds-domain" => {
                let v = it.next().context("--dds-domain requer um valor")?;
                dds_domain = v.parse().context("--dds-domain inválido")?;
            }
            "--dds-secure" => {
                dds_secure = true;
            }
            "--dds-security-dir" => {
                dds_security_dir = Some(it.next().context("--dds-security-dir requer um valor")?);
            }
            // --sandbox-dir é o nome histórico do Python; --filesystem-root é o alias.
            "--filesystem-root" | "--sandbox-dir" => {
                filesystem_root = it.next().context("--filesystem-root requer um valor")?;
            }
            "-h" | "--help" => {
                eprintln!(
                    "mcp-gateway — DDS <-> ferramentas MCP\n\
                     Uso: mcp-gateway [--dds-domain N] [--filesystem-root DIR] [--dds-secure] [--dds-security-dir DIR]\n\
                     Default: domínio 0, raiz /tmp/sandbox, deny até snapshot válido."
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("argumento desconhecido: '{other}' (veja --help)"),
        }
    }

    Ok(Args {
        dds_domain,
        dds_secure,
        dds_security_dir,
        filesystem_root,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    if args.dds_secure {
        #[cfg(not(feature = "security"))]
        anyhow::bail!("--dds-secure requires the security feature to be enabled at build time");
    } else {
        tracing::warn!(
            "DDS running in local-only mode without authentication or encryption; \
             do not expose this deployment to untrusted networks"
        );
    }

    #[cfg(feature = "security")]
    let service = {
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
        mcp_gateway::dds::build_service_with_security(
            args.dds_domain,
            &args.filesystem_root,
            security,
        )?
    };
    #[cfg(not(feature = "security"))]
    let service = mcp_gateway::dds::build_service(args.dds_domain, &args.filesystem_root)?;
    eprintln!(
        "mcp-gateway: dominio={} raiz={} tools={:?} — aguardando ToolCall.Request",
        args.dds_domain,
        args.filesystem_root,
        service.registry().list_tools()
    );

    tokio::select! {
        r = service.run() => r?,
        _ = tokio::signal::ctrl_c() => eprintln!("mcp-gateway: SIGINT/SIGTERM — encerrando"),
    }
    Ok(())
}
