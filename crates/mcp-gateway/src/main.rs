//! Binário `mcp-gateway` (feature `dds`): sobe o serviço no domínio e aguarda.
//!
//! Equivalente ao `main.py` Python:
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p mcp-gateway --features dds -- \
//!     --dds-domain 0 --filesystem-root /tmp/sandbox
//! # com política de nível máximo (default: permissiva):
//! CYCLONEDDS_STATIC=1 cargo run -p mcp-gateway --features dds -- --max-security-level 1
//! ```

use anyhow::{Context, Result};
use mcp_gateway::policy::{PermissivePolicy, PolicyHook, SecurityPolicy};
use std::sync::Arc;

struct Args {
    dds_domain: u32,
    filesystem_root: String,
    max_security_level: Option<i32>,
}

fn parse_args() -> Result<Args> {
    let mut dds_domain = 0u32;
    let mut filesystem_root = "/tmp/sandbox".to_string();
    let mut max_security_level = None;

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
            "--max-security-level" => {
                let v = it.next().context("--max-security-level requer um valor")?;
                max_security_level = Some(v.parse().context("--max-security-level inválido")?);
            }
            "-h" | "--help" => {
                eprintln!(
                    "mcp-gateway — DDS <-> ferramentas MCP\n\
                     Uso: mcp-gateway [--dds-domain N] [--filesystem-root DIR]\n\
                     \x20       [--max-security-level 0..3]\n\
                     Default: domínio 0, raiz /tmp/sandbox, política permissiva."
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("argumento desconhecido: '{other}' (veja --help)"),
        }
    }

    Ok(Args {
        dds_domain,
        filesystem_root,
        max_security_level,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;

    // Default permissivo (como o gateway Python sem policy carregada); com
    // --max-security-level, aplica o fast-path do PolicyEngine Python.
    let policy: Arc<dyn PolicyHook> = match args.max_security_level {
        Some(max) => Arc::new(SecurityPolicy {
            max_security_level: max,
            ..Default::default()
        }),
        None => Arc::new(PermissivePolicy),
    };

    let service = mcp_gateway::dds::build_service(args.dds_domain, &args.filesystem_root, policy)?;
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
