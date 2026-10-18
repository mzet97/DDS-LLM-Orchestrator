//! Status de hosts do cluster (paridade reduzida com `host_status.py`):
//! sonda de alcançabilidade SSH/local com cache TTL de 30 s.
//! Facts profundos via Ansible (`setup`) ficam fora: aqui `online` significa
//! porta SSH aberta (ou host local), nunca facts inventados.

use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::inventory::VmHost;

const TTL: Duration = Duration::from_secs(30);

/// Status sondado de um host.
#[derive(Debug, Clone, Serialize)]
pub struct HostStatus {
    pub name: String,
    pub address: String,
    pub online: bool,
    pub connection: Option<String>,
    pub ansible_user: Option<String>,
    pub groups: Vec<String>,
    pub checked_at_unix: u64,
    pub detail: String,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Sonda um host: local => online; remoto => TCP 22 em até 2 s.
pub fn probe_host(host: &VmHost) -> HostStatus {
    let local = host.connection.as_deref() == Some("local") || host.address == "localhost";
    let (online, detail) = if local {
        (true, "host local".to_string())
    } else {
        let addr = format!("{}:22", host.address);
        match addr.parse::<SocketAddr>() {
            Ok(socket) => match TcpStream::connect_timeout(&socket, Duration::from_secs(2)) {
                Ok(_) => (true, "ssh (tcp/22) aberto".to_string()),
                Err(e) => (false, format!("ssh inacessivel: {e}")),
            },
            Err(_) => (false, "endereco invalido para sonda tcp".to_string()),
        }
    };
    HostStatus {
        name: host.name.clone(),
        address: host.address.clone(),
        online,
        connection: host.connection.clone(),
        ansible_user: host.ansible_user.clone(),
        groups: host.groups.clone(),
        checked_at_unix: now_unix(),
        detail,
    }
}

/// Cache TTL por host (HostStatusCache do Python).
pub struct HostStatusCache {
    entries: Mutex<BTreeMap<String, (Instant, HostStatus)>>,
}

impl HostStatusCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    /// Status de todos os hosts (usa cache dentro do TTL).
    pub fn get_all(&self, hosts: &[VmHost]) -> Vec<HostStatus> {
        hosts.iter().map(|h| self.get_one(h, false)).collect()
    }

    /// Status de um host; `force` ignora o cache.
    pub fn get_one(&self, host: &VmHost, force: bool) -> HostStatus {
        if !force {
            let entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((when, status)) = entries.get(&host.name) {
                if when.elapsed() < TTL {
                    return status.clone();
                }
            }
        }
        let status = probe_host(host);
        self.entries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(host.name.clone(), (Instant::now(), status.clone()));
        status
    }

    pub fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }
}

impl Default for HostStatusCache {
    fn default() -> Self {
        Self::new()
    }
}
