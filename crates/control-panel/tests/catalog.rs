//! Paridade com `src/control_panel/tests/test_service_catalog.py`.

use std::collections::BTreeMap;

use control_panel::catalog::{get_agent_services, get_service_catalog, OverrideValue};
use control_panel::config::PanelConfig;
use control_panel::presets::{get_preset_by_id, get_presets, resolve_preset_services};

fn cfg() -> PanelConfig {
    PanelConfig::from_env()
}

#[test]
fn catalog_contains_rust_core_services() {
    let catalog = get_service_catalog(&cfg());
    for name in ["orchestrator", "studio_node", "llama_server"] {
        let svc = catalog.get(name).expect("servico no catalogo");
        assert_eq!(svc.name, name);
        assert!(!svc.command.is_empty());
        // Atualização da fase 910: nenhum serviço sobe módulo Python.
        assert!(!svc
            .command
            .iter()
            .any(|a| a.contains("python") || a.ends_with(".py")));
    }
}

#[test]
fn agent_services_generated() {
    let agents = get_agent_services(&cfg(), 3, "dds", 2);
    assert_eq!(agents.len(), 3);
    for (name, svc) in &agents {
        assert!(name.starts_with("agent_"));
        assert!(svc.command.iter().any(|a| a == "--slots"));
        assert!(svc.command.iter().any(|a| a == "--agent-id"));
    }
}

#[test]
fn presets_have_known_ids() {
    let ids: Vec<String> = get_presets().iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&"minimal".to_string()));
    assert!(ids.contains(&"local-dds".to_string()));
    assert!(ids.contains(&"full-rust".to_string()));
}

#[test]
fn resolve_minimal_preset() {
    let preset = get_preset_by_id("minimal").expect("preset minimal");
    let resolved = resolve_preset_services(&cfg(), &preset).expect("resolve");
    assert!(resolved.contains_key("orchestrator"));
    assert_eq!(resolved.len(), 1);
}

#[test]
fn resolve_local_dds_preset_has_agents() {
    let preset = get_preset_by_id("local-dds").expect("preset local-dds");
    let resolved = resolve_preset_services(&cfg(), &preset).expect("resolve");
    assert!(resolved.keys().filter(|n| n.starts_with("agent_")).count() >= 3);
    assert!(resolved.contains_key("orchestrator"));
}

#[test]
fn build_command_with_overrides() {
    let catalog = get_service_catalog(&cfg());
    let svc = catalog.get("orchestrator").expect("orchestrator");
    let overrides = BTreeMap::from([
        ("dds_domain".to_string(), OverrideValue::Int(42)),
        ("log_level".to_string(), OverrideValue::Text("DEBUG".into())),
    ]);
    let cmd = svc.build_command(&overrides);
    let idx = cmd.iter().position(|a| a == "--dds-domain").expect("flag");
    assert_eq!(cmd[idx + 1], "42");
    let idx = cmd
        .iter()
        .position(|a| a == "--log-level")
        .expect("flag nova");
    assert_eq!(cmd[idx + 1], "DEBUG");
}

#[test]
fn build_command_boolean_flag() {
    let catalog = get_service_catalog(&cfg());
    let svc = catalog.get("orchestrator").expect("orchestrator");
    let off = svc.build_command(&BTreeMap::from([(
        "enable_agent_selection".to_string(),
        OverrideValue::Bool(false),
    )]));
    assert!(!off.iter().any(|a| a == "--enable-agent-selection"));
    let on = svc.build_command(&BTreeMap::from([(
        "enable_agent_selection".to_string(),
        OverrideValue::Bool(true),
    )]));
    assert!(on.iter().any(|a| a == "--enable-agent-selection"));
}
