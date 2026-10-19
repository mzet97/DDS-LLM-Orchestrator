//! Parte de `discovery` (extraída para manter os arquivos revisáveis).

use super::*;

/// Corredor do worker (feature `dds`): runtime tokio de thread + stream de
/// presença contínuo; nó novo → probe automático; snapshot a cada evento.
#[cfg(feature = "dds")]
pub(super) fn run_worker(domain: u32, tx: std::sync::mpsc::Sender<DiscoverySnapshot>) {
    crate::studio_log::info(format!("descoberta: worker iniciado (domínio {domain})"));
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            let _ = tx.send(DiscoverySnapshot::default());
            crate::studio_log::error(format!("descoberta: runtime tokio: {err}"));
            return;
        }
    };
    rt.block_on(async move {
        let space = match dds_dataspace::DataSpace::new(domain, 0) {
            Ok(space) => std::sync::Arc::new(space),
            Err(err) => {
                crate::studio_log::error(format!("descoberta: DataSpace domínio {domain}: {err}"));
                let _ = tx.send(DiscoverySnapshot::default());
                return;
            }
        };
        use futures::StreamExt;
        use std::pin::pin;
        // Inventário vivo do domínio: instalações (NodePresence), agentes
        // (AgentRegistry) e inferência (ServerStatus) — mesmo worker, um
        // snapshot único para a GUI inteira.
        let mut presences = pin!(space.stream_studio_node_presences());
        let mut agents_stream = pin!(space.stream_agent_states());
        let mut servers_stream = pin!(space.stream_server_statuses());
        let mut nodes: Vec<DiscoveredNode> = Vec::new();
        let mut agents: std::collections::BTreeMap<String, AgentState> =
            std::collections::BTreeMap::new();
        let mut servers: std::collections::BTreeMap<String, (ServerStatus, u64)> =
            std::collections::BTreeMap::new();
        crate::studio_log::info(
            "descoberta: escutando NodePresence + AgentRegistry + ServerStatus",
        );
        let home = std::env::var("HOME").ok();
        enum Event {
            Presence(dds_contract::generated::dds_llm_orchestrator::StudioNodePresence),
            Agent(AgentState),
            Server(ServerStatus),
        }
        loop {
            let event = tokio::select! {
                Some(p) = presences.next() => Event::Presence((*p).clone()),
                Some(a) = agents_stream.next() => Event::Agent((*a).clone()),
                Some(sv) = servers_stream.next() => Event::Server((*sv).clone()),
                else => break,
            };
            match event {
                Event::Presence(presence) => {
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
                        crate::studio_log::info(format!(
                            "descoberta: NOVO nó {} @ {} → probe: {}",
                            presence.node_id,
                            presence.url,
                            probe.as_ref().map(|p| p.detail.as_str()).unwrap_or("?")
                        ));
                        if let Some(entry) =
                            nodes.iter_mut().find(|n| n.node_id == presence.node_id)
                        {
                            entry.probe = probe;
                        }
                    }
                }
                Event::Agent(agent) => {
                    let is_new_agent = !agents.contains_key(&agent.agent_id);
                    if is_new_agent {
                        crate::studio_log::info(format!(
                            "descoberta: agente {} (modelo {}, slots {}/{})",
                            agent.agent_id, agent.model, agent.slots_busy, agent.slots_total
                        ));
                    }
                    agents.insert(agent.agent_id.clone(), agent);
                }
                Event::Server(server) => {
                    let is_new_server = !servers.contains_key(&server.server_id);
                    if is_new_server {
                        crate::studio_log::info(format!(
                            "descoberta: inferência {} (modelo {}, pronto={})",
                            server.server_id, server.model_loaded, server.ready
                        ));
                    }
                    servers.insert(server.server_id.clone(), (server, now_unix_ns()));
                }
            }
            // Poda honesta: agente/inferência sem heartbeat há >30 s sai do
            // inventário. `ServerStatus` não tem relógio no fio — o lease
            // conta do recebimento local (P2: antes nunca era podado).
            let now = now_unix_ns();
            let before = agents.len();
            agents.retain(|_, a| now.saturating_sub(a.last_update_ns) < HEARTBEAT_LEASE_NS);
            if agents.len() < before {
                crate::studio_log::warn(format!(
                    "descoberta: {} agente(s) podado(s) por heartbeat >30 s",
                    before - agents.len()
                ));
            }
            let pruned_servers = retain_fresh(&mut servers, now);
            if pruned_servers > 0 {
                crate::studio_log::warn(format!(
                    "descoberta: {pruned_servers} servidor(es) podado(s) por heartbeat >30 s"
                ));
            }
            let _ = tx.send(DiscoverySnapshot {
                nodes: nodes.clone(),
                agents: agents.values().map(agent_row).collect(),
                servers: servers
                    .values()
                    .map(|(status, _)| server_row(status))
                    .collect(),
            });
        }
    });
}

/// Probe automático de `/version` (blocking — roda na thread do worker).
#[cfg(feature = "dds")]
pub(super) fn probe_node(url: &str, token: Option<&str>) -> ProbeStatus {
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
