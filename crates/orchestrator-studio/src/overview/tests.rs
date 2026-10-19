use studio_node::protocol::{ProtocolVersion, NODE_PROTOCOL_VERSION};

use super::*;

fn input<'a>(
    node: Option<&'a NodeSummary>,
    services: &'a [ServiceStatus],
    agents: &'a [AgentInfo],
) -> OverviewInput<'a> {
    OverviewInput {
        node,
        node_error: "",
        services,
        services_error: "",
        agents,
        agents_error: "",
        models_total: 0,
        models_hashed: 0,
        inference_proof: "",
        discovery_nodes: 0,
        discovery_agents: 0,
        discovery_servers: 0,
        discovery_target: None,
        catalog_items: 0,
        catalog_cursor: 0,
        catalog_loaded: false,
        catalog_error: "",
    }
}

#[test]
fn empty_everything_is_stale_never_fake() {
    let data = input(None, &[], &[]);

    let tiles = summarize(&data);

    assert_eq!(tiles.len(), 6);
    assert!(tiles.iter().all(|tile| tile.stale));
}

#[test]
fn counts_come_from_real_slices() {
    let node = NodeSummary {
        version: ProtocolVersion {
            major: NODE_PROTOCOL_VERSION.major,
            minor: NODE_PROTOCOL_VERSION.minor,
        },
        operations: Vec::new(),
    };
    let services = vec![
        ServiceStatus {
            service: String::from("a"),
            wanted: Some(true),
            active: true,
        },
        ServiceStatus {
            service: String::from("b"),
            wanted: Some(true),
            active: false,
        },
    ];
    let agents = vec![AgentInfo {
        agent_id: String::from("ag-1"),
        model: String::new(),
        specialization: String::new(),
        hostname: String::new(),
        health: 100,
        slots_busy: 1,
        slots_total: 4,
        completed_total: 0,
        failed_total: 0,
        ema_latency_ms: 0.0,
    }];
    let mut data = input(Some(&node), &services, &agents);
    data.models_total = 15;
    data.models_hashed = 4;
    data.inference_proof = "OK";
    data.catalog_items = 14;
    data.catalog_cursor = 49;
    data.catalog_loaded = true;

    let tiles = summarize(&data);

    assert!(tiles.iter().all(|tile| !tile.stale));
    let services_tile = tiles
        .iter()
        .find(|tile| tile.title == "Serviços")
        .expect("servicos");
    assert!(services_tile.summary.contains("1 pronto(s) de 2"));
    assert!(services_tile.summary.contains("1 divergente(s)"));
    let agents_tile = tiles
        .iter()
        .find(|tile| tile.title == "Agentes")
        .expect("agentes");
    assert!(agents_tile.summary.contains("slots 1/4"));
}

#[test]
fn source_errors_stay_visible() {
    let mut data = input(None, &[], &[]);
    data.node_error = "conexão recusada";
    data.services_error = "timeout";

    let tiles = summarize(&data);

    let node = tiles.iter().find(|tile| tile.title == "Nó").expect("no");
    assert!(node.summary.contains("conexão recusada"));
    let services = tiles
        .iter()
        .find(|tile| tile.title == "Serviços")
        .expect("servicos");
    assert!(services.summary.contains("timeout"));
}
