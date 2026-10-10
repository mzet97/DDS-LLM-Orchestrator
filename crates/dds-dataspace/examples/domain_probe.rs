//! Sonda de verdade do domínio: o que REALMENTE está anunciando
//! (nós/agentes/inferência) em N segundos.
use futures::StreamExt;

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
    let space = dds_dataspace::DataSpace::new(domain, 0)?;
    eprintln!("[probe] domínio {domain} por {secs}s…");
    use std::pin::pin;
    let mut nodes = pin!(space.stream_studio_node_presences());
    let mut agents = pin!(space.stream_agent_states());
    let mut servers = pin!(space.stream_server_statuses());
    let mut seen_n: std::collections::BTreeSet<String> = Default::default();
    let mut seen_a: std::collections::BTreeSet<String> = Default::default();
    let mut seen_s: std::collections::BTreeSet<String> = Default::default();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(left) => break,
            Some(p) = nodes.next() => { seen_n.insert(format!("{} @ {} (token={})", p.node_id, p.url, p.token_required)); }
            Some(a) = agents.next() => { seen_a.insert(format!("{} model={} slots={}/{} hb_age={:.0}s", a.agent_id, a.model, a.slots_busy, a.slots_total, (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64).saturating_sub(a.last_update_ns) as f64 / 1e9)); }
            Some(s) = servers.next() => { seen_s.insert(format!("{} model={} ready={}", s.server_id, s.model_loaded, s.ready)); }
        }
    }
    eprintln!("[probe] NÓS ({}):", seen_n.len());
    for v in &seen_n {
        eprintln!("  {v}");
    }
    eprintln!("[probe] AGENTES ({}):", seen_a.len());
    for v in &seen_a {
        eprintln!("  {v}");
    }
    eprintln!("[probe] INFERÊNCIA ({}):", seen_s.len());
    for v in &seen_s {
        eprintln!("  {v}");
    }
    Ok(())
}
