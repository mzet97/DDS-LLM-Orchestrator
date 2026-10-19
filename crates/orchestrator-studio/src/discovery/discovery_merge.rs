//! Parte de `discovery` (extraída para manter os arquivos revisáveis).

#[cfg(any(test, feature = "dds"))]
use super::*;

/// Lease de heartbeat do inventário (30 s — agentes E inferência): sem
/// sinal nesse intervalo, a entrada sai do snapshot (poda honesta).
#[cfg(any(test, feature = "dds"))]
pub(super) const HEARTBEAT_LEASE_NS: u64 = 30_000_000_000;

/// Remove entradas cujo recebimento local envelheceu além do lease;
/// devolve quantas saíram. `ServerStatus` não carrega relógio no fio —
/// o par `(valor, recebido_local_ns)` é registrado na inserção (P2).
#[cfg(any(test, feature = "dds"))]
pub(super) fn retain_fresh<T>(
    entries: &mut std::collections::BTreeMap<String, (T, u64)>,
    now_ns: u64,
) -> usize {
    let before = entries.len();
    entries.retain(|_, (_, received_ns)| now_ns.saturating_sub(*received_ns) < HEARTBEAT_LEASE_NS);
    before - entries.len()
}

/// Mescla uma amostra de presença na lista: atualiza a entrada do
/// `node_id` (ou insere, marcando `novo_url` para o probe automático).
#[cfg(any(test, feature = "dds"))]
pub fn merge_presence(
    nodes: &mut Vec<DiscoveredNode>,
    node_id: &str,
    url: &str,
    token_required: bool,
    last_seen_unix_ns: u64,
) -> bool {
    if let Some(entry) = nodes.iter_mut().find(|n| n.node_id == node_id) {
        entry.url = url.to_owned();
        entry.token_required = token_required;
        entry.last_seen_unix_ns = last_seen_unix_ns;
        return false;
    }
    nodes.push(DiscoveredNode {
        node_id: node_id.to_owned(),
        url: url.to_owned(),
        token_required,
        last_seen_unix_ns,
        probe: None,
    });
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    true
}

/// Token salvo para o host de uma URL (`~/.config/studio/hosts/<host>.token`
/// — o mesmo arquivo do deploy T-840-02). `None` sem arquivo ou sem HOME.
#[must_use]
pub fn token_for_url(url: &str, home: Option<&str>) -> Option<String> {
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split(':')
        .next()?
        .to_owned();
    let home = home?;
    let path = std::path::Path::new(home)
        .join(".config/studio/hosts")
        .join(format!("{host}.token"));
    std::fs::read_to_string(path)
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

#[cfg(feature = "dds")]
impl DiscoveryState {
    /// Sobe o worker de descoberta contínua (chamado no boot da GUI).
    pub fn start(domain: u32) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || run_worker(domain, tx));
        Self {
            domain,
            scanning: true,
            receiver: Some(rx),
            ..Self::default()
        }
    }
}
