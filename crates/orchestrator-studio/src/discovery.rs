//! Descoberta automática de instalações (T-890-03, G-70/44): ao abrir o
//! Studio, um worker em background escuta o tópico `Studio.NodePresence`
//! (19º tópico) no domínio informado e mantém a lista de instalações vivas —
//! sem nenhum clique. Cada nó novo recebe probe automático de `/version`
//! com o token salvo em `~/.config/studio/hosts/<host>.token`, quando houver.
//!
//! Com a feature `dds`, [`DiscoveryState::start`] sobe o worker (thread +
//! runtime tokio próprio, padrão da GUI). Sem ela, o estado existe e a UI
//! explica como habilitar.

#[cfg(feature = "dds")]
use crate::machines::ProbeState;
use crate::machines::ProbeStatus;

/// Uma instalação descoberta via `Studio.NodePresence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredNode {
    pub node_id: String,
    pub url: String,
    pub token_required: bool,
    /// Último heartbeat recebido (ns UNIX) — idade > lease (10 s) = morto.
    pub last_seen_unix_ns: u64,
    /// Probe automático de `/version` (preenchido quando o nó é novo).
    pub probe: Option<ProbeStatus>,
}

impl DiscoveredNode {
    /// Idade do último heartbeat em segundos (relógio local).
    #[must_use]
    pub fn age_secs(&self, now_unix_ns: u64) -> u64 {
        now_unix_ns.saturating_sub(self.last_seen_unix_ns) / 1_000_000_000
    }

    /// Vivo = heartbeat dentro do lease do tópico (10 s, dobrado por
    /// tolerância de relógio entre máquinas).
    #[must_use]
    pub fn is_alive(&self, now_unix_ns: u64) -> bool {
        self.age_secs(now_unix_ns) <= 20
    }
}

/// Evento do worker para a UI: foto completa das instalações conhecidas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverySnapshot {
    pub nodes: Vec<DiscoveredNode>,
}

/// Estado do painel de descoberta (thread-safe via mpsc — padrão da GUI).
#[derive(Default)]
pub struct DiscoveryState {
    pub domain: u32,
    pub nodes: Vec<DiscoveredNode>,
    /// `true` até o primeiro snapshot chegar (worker ainda escaneando).
    pub scanning: bool,
    pub error: String,
    #[cfg(feature = "dds")]
    receiver: Option<std::sync::mpsc::Receiver<DiscoverySnapshot>>,
}

impl DiscoveryState {
    /// Estado sem worker (feature `dds` desligada — a UI explica).
    #[must_use]
    pub fn new_disabled(domain: u32) -> Self {
        Self {
            domain,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.scanning
    }

    /// Drena o snapshot mais recente do worker (chamar por frame).
    pub fn poll(&mut self) {
        #[cfg(feature = "dds")]
        if let Some(rx) = &self.receiver {
            while let Ok(snapshot) = rx.try_recv() {
                self.nodes = snapshot.nodes;
                self.scanning = false;
                self.error.clear();
            }
        }
    }

    /// Observa um snapshot direto (mesma transição do `poll`, testável).
    pub fn observe(&mut self, nodes: Vec<DiscoveredNode>) {
        self.nodes = nodes;
        self.scanning = false;
    }
}

/// Mescla uma amostra de presença na lista: atualiza a entrada do
/// `node_id` (ou insere, marcando `novo_url` para o probe automático).
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

/// Corredor do worker (feature `dds`): runtime tokio de thread + stream de
/// presença contínuo; nó novo → probe automático; snapshot a cada evento.
#[cfg(feature = "dds")]
fn run_worker(domain: u32, tx: std::sync::mpsc::Sender<DiscoverySnapshot>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            let _ = tx.send(DiscoverySnapshot { nodes: Vec::new() });
            eprintln!("studio: descoberta: runtime tokio: {err}");
            return;
        }
    };
    rt.block_on(async move {
        let space = match dds_dataspace::DataSpace::new(domain, 0) {
            Ok(space) => std::sync::Arc::new(space),
            Err(err) => {
                eprintln!("studio: descoberta: DataSpace domínio {domain}: {err}");
                let _ = tx.send(DiscoverySnapshot { nodes: Vec::new() });
                return;
            }
        };
        use futures::StreamExt;
        use std::pin::pin;
        let mut stream = pin!(space.stream_studio_node_presences());
        let mut nodes: Vec<DiscoveredNode> = Vec::new();
        let home = std::env::var("HOME").ok();
        while let Some(presence) = stream.next().await {
            let is_new = merge_presence(
                &mut nodes,
                &presence.node_id,
                &presence.url,
                presence.token_required,
                presence.last_seen_ns,
            );
            if is_new {
                let token = crate::discovery::token_for_url(&presence.url, home.as_deref());
                let probe = Some(probe_node(&presence.url, token.as_deref()));
                if let Some(entry) = nodes.iter_mut().find(|n| n.node_id == presence.node_id) {
                    entry.probe = probe;
                }
            }
            let _ = tx.send(DiscoverySnapshot {
                nodes: nodes.clone(),
            });
        }
    });
}

/// Probe automático de `/version` (blocking — roda na thread do worker).
#[cfg(feature = "dds")]
fn probe_node(url: &str, token: Option<&str>) -> ProbeStatus {
    match crate::origin::fetch_node_version_with_token(url, token) {
        Ok(version) => ProbeStatus {
            state: ProbeState::Online,
            detail: format!("protocolo {}.{}", version.major, version.minor),
        },
        Err(crate::origin::OriginError::Unauthorized) => ProbeStatus {
            state: ProbeState::AuthPending,
            detail: match token {
                Some(_) => String::from("token recusado (401)"),
                None => String::from("token ausente (nó respondeu 401)"),
            },
        },
        Err(err) => ProbeStatus {
            state: ProbeState::Offline,
            detail: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_presence, token_for_url, DiscoveredNode, DiscoveryState};

    #[test]
    fn merge_updates_existing_and_sorts_new_entries() {
        let mut nodes = Vec::new();
        assert!(
            merge_presence(&mut nodes, "b:1", "http://b:1", true, 100),
            "primeira inserção é nova"
        );
        assert!(!merge_presence(&mut nodes, "b:1", "http://b:1", true, 200));
        assert_eq!(nodes[0].last_seen_unix_ns, 200, "heartbeat atualiza");

        assert!(merge_presence(&mut nodes, "a:1", "http://a:1", false, 50));
        let ids: Vec<_> = nodes.iter().map(|n| n.node_id.as_str()).collect();
        assert_eq!(ids, vec!["a:1", "b:1"], "lista ordenada por node_id");
    }

    #[test]
    fn alive_window_follows_lease_with_clock_tolerance() {
        let now = 1_000_000_000_000u64;
        let node = |last_seen: u64| DiscoveredNode {
            node_id: String::from("n"),
            url: String::from("http://n:4317"),
            token_required: true,
            last_seen_unix_ns: last_seen,
            probe: None,
        };
        assert!(node(now - 5_000_000_000).is_alive(now), "5 s = vivo");
        assert!(!node(now - 30_000_000_000).is_alive(now), "30 s = morto");
    }

    #[test]
    fn token_for_url_reads_hosts_dir() {
        let home = std::env::temp_dir().join(format!(
            "studio-token-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let dir = home.join(".config/studio/hosts");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("192.168.1.64.token"), "segredo-123\n").expect("token");

        assert_eq!(
            token_for_url(
                "http://192.168.1.64:4317",
                Some(home.to_str().expect("path"))
            ),
            Some(String::from("segredo-123"))
        );
        assert_eq!(
            token_for_url("http://10.0.0.1:4317", Some(home.to_str().expect("path"))),
            None
        );
        assert_eq!(token_for_url("http://10.0.0.1:4317", None), None);
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn observe_replaces_snapshot_and_clears_scanning() {
        let mut state = DiscoveryState::new_disabled(170);
        state.scanning = true;
        state.observe(vec![DiscoveredNode {
            node_id: String::from("n:1"),
            url: String::from("http://n:4317"),
            token_required: false,
            last_seen_unix_ns: 1,
            probe: None,
        }]);
        assert!(!state.scanning);
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.nodes[0].node_id, "n:1");
    }
}
