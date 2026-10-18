//! Parte de `kit` (extraída para manter os arquivos revisáveis).

/// Host curto do design (".61" de "http://192.168.1.61:4317"); fora de IPv4
/// cai para a URL truncada.
pub fn short_host(url: &str) -> String {
    let authority = url.rsplit("://").next().unwrap_or(url);
    let host = authority.split(':').next().unwrap_or(authority);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|part| part.parse::<u8>().is_ok()) {
        format!(".{}", parts[3])
    } else {
        url.chars().take(12).collect()
    }
}

/// Autoridade curta (host:port sem esquema nem caminho).
pub fn authority(url: &str) -> String {
    url.rsplit("://")
        .next()
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .chars()
        .take(24)
        .collect()
}

/// Agente curto do design ("lab-01" de "agent-lab-01").
pub fn short_agent(agent_id: &str) -> String {
    agent_id
        .strip_prefix("agent-")
        .unwrap_or(agent_id)
        .chars()
        .take(14)
        .collect()
}

/// ms unix → HH:MM:SS.mmm (relógios de auditoria/sync; conta manual).
#[must_use]
pub fn clock_ms(ts_unix_ms: u64) -> String {
    let secs = ts_unix_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        ts_unix_ms % 1000,
    )
}
