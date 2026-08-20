//! Binário `mcp-gateway` (feature `dds`): sobe o serviço no domínio e aguarda.
//!
//! Equivalente ao `main.py` Python:
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p mcp-gateway --features dds -- \
//!     --dds-domain 0 --filesystem-root /tmp/sandbox
//! ```

use anyhow::{Context, Result};
struct Args {
    dds_domain: u32,
    filesystem_root: String,
}

fn parse_args() -> Result<Args> {
    let mut dds_domain = 0u32;
    let mut filesystem_root = "/tmp/sandbox".to_string();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dds-domain" => {
                let v = it.next().context("--dds-domain requer um valor")?;
                dds_domain = v.parse().context("--dds-domain inválido")?;
            }
            // --sandbox-dir é o nome histórico do Python; --filesystem-root é o alias.
            "--filesystem-root" | "--sandbox-dir" => {
                filesystem_root = it.next().context("--filesystem-root requer um valor")?;
            }
            "-h" | "--help" => {
                eprintln!(
                    "mcp-gateway — DDS <-> ferramentas MCP\n\
                     Uso: mcp-gateway [--dds-domain N] [--filesystem-root DIR]\n\
                     Default: domínio 0, raiz /tmp/sandbox, deny até snapshot válido."
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("argumento desconhecido: '{other}' (veja --help)"),
        }
    }

    Ok(Args {
        dds_domain,
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
