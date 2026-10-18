//! Presets do painel Rust (Preset/resolve_preset_services do Python).

use std::collections::BTreeMap;

use crate::catalog::{agent_service, get_service_catalog, Preset, ServiceDefinition};
use crate::config::PanelConfig;

/// Presets do painel Rust.
pub fn get_presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "minimal".into(),
            name: "Minimo (orchestrator)".into(),
            description: "Apenas o orquestrador Rust.".into(),
            services: vec!["orchestrator".into()],
            agent_ids: vec![],
        },
        Preset {
            id: "local-dds".into(),
            name: "DDS Local (orchestrator + agentes Rust)".into(),
            description: "Orquestrador + 3 agentes Rust (engine dds/http conforme ambiente)."
                .into(),
            services: vec!["orchestrator".into()],
            agent_ids: vec![
                "agent-dds-1".into(),
                "agent-dds-2".into(),
                "agent-dds-3".into(),
            ],
        },
        Preset {
            id: "full-rust".into(),
            name: "Full Rust".into(),
            description: "Orquestrador + studio-node + llama-server + 2 agentes.".into(),
            services: vec![
                "orchestrator".into(),
                "studio_node".into(),
                "llama_server".into(),
            ],
            agent_ids: vec!["agent-full-1".into(), "agent-full-2".into()],
        },
    ]
}

pub fn get_preset_by_id(id: &str) -> Option<Preset> {
    get_presets().into_iter().find(|p| p.id == id)
}

/// Converte um preset em definições concretas (resolve_preset_services).
pub fn resolve_preset_services(
    cfg: &PanelConfig,
    preset: &Preset,
) -> Result<BTreeMap<String, ServiceDefinition>, String> {
    let catalog = get_service_catalog(cfg);
    let mut resolved = BTreeMap::new();
    for name in &preset.services {
        let svc = catalog
            .get(name)
            .ok_or_else(|| format!("Servico desconhecido no preset: {name}"))?;
        resolved.insert(name.clone(), svc.clone());
    }
    for agent_id in &preset.agent_ids {
        let engine = std::env::var("CP_AGENT_ENGINE").unwrap_or_else(|_| "dds".into());
        let svc = agent_service(cfg, agent_id, &engine, 1);
        resolved.insert(svc.name.clone(), svc);
    }
    Ok(resolved)
}
