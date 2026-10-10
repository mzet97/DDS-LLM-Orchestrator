//! Leitor headless de presença (T-890): lista instalações do Studio vivas no
//! domínio via o 19º tópico `Studio.NodePresence` (TransitLocal — instâncias
//! sobrevivem ao publisher; `alive` = lease de 10 s de heartbeat).
//! `cargo run --release -p dds-dataspace --features dds --example list_node_presence -- [domínio] [segundos]`

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let domain: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(170);
    let secs: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(12);
    let dataspace = Arc::new(dds_dataspace::DataSpace::new(domain, 0)?);
    eprintln!("[presenca] ouvindo Studio.NodePresence no domínio {domain} por {secs}s...");
    use futures::StreamExt; // StreamExt + pin-project: o stream de presença não é Unpin
    let mut stream = std::pin::pin!(dataspace.stream_studio_node_presences());
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
    let mut seen = std::collections::HashMap::new();
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout_at(deadline, stream.next()).await {
            Ok(Some(p)) => {
                let now_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                let age_s = now_ns.saturating_sub(p.last_seen_ns) as f64 / 1e9;
                // lease do tópico = 10 s de heartbeat: idade > 10 s = instalação morta
                let alive = age_s <= 10.0;
                eprintln!(
                    "[presenca] node_id={} url={} token={} services=\"{}\" idade_last_seen={age_s:.1}s alive={alive}",
                    p.node_id, p.url, p.token_required, p.services_hint
                );
                seen.insert(p.node_id.clone(), alive);
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    if seen.is_empty() {
        eprintln!("[presenca] nenhuma instalação recebida em {secs}s");
    } else {
        eprintln!(
            "[presenca] {} instalação(ões) vista(s), {} viva(s)",
            seen.len(),
            seen.values().filter(|a| **a).count()
        );
    }
    Ok(())
}
