//! Catálogo de serviços e presets apontando para os binários Rust atuais.
//! Paridade comportamental com `service_catalog.py` (build_command, presets,
//! agentes gerados), com comandos atualizados: nada de `python -m`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::PanelConfig;

/// Valor de override de argumento de comando.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OverrideValue {
    Bool(bool),
    Int(i64),
    Text(String),
}

impl OverrideValue {
    fn as_flag_value(&self) -> String {
        match self {
            OverrideValue::Bool(b) => b.to_string(),
            OverrideValue::Int(i) => i.to_string(),
            OverrideValue::Text(s) => s.clone(),
        }
    }
}

/// Descrição de um serviço gerenciável (ServiceDefinition do Python).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub auto_restart: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub overridable_args: BTreeMap<String, OverrideValue>,
}

impl ServiceDefinition {
    /// Monta o comando final (service_catalog.py:36-56): flag existente tem o
    /// valor substituído; booleano adiciona/remove a flag; demais são anexados.
    pub fn build_command(&self, overrides: &BTreeMap<String, OverrideValue>) -> Vec<String> {
        let mut cmd = self.command.clone();
        for (key, value) in overrides {
            let flag = format!("--{}", key.replace('_', "-"));
            if let OverrideValue::Bool(on) = value {
                if *on {
                    if !cmd.iter().any(|a| a == &flag) {
                        cmd.push(flag);
                    }
                } else {
                    cmd.retain(|a| a != &flag);
                }
                continue;
            }
            let text = value.as_flag_value();
            if let Some(idx) = cmd.iter().position(|a| a == &flag) {
                if idx + 1 < cmd.len() {
                    cmd[idx + 1] = text;
                } else {
                    cmd.push(text);
                }
            } else {
                cmd.push(flag);
                cmd.push(text);
            }
        }
        cmd
    }
}

/// Conjunto nomeado de serviços (Preset do Python).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Serviços singleton do catálogo.
    pub services: Vec<String>,
    /// Agentes dinâmicos adicionais (agent_id prefixados).
    #[serde(default)]
    pub agent_ids: Vec<String>,
}

fn bin(name: &str, env_key: &str) -> String {
    std::env::var(env_key).unwrap_or_else(|_| name.to_string())
}

fn orchestrator(cfg: &PanelConfig) -> ServiceDefinition {
    ServiceDefinition {
        name: "orchestrator".into(),
        display_name: "Orchestrator (Rust)".into(),
        description: "Monitor/reaper data-centric DDS (binario Rust atual).".into(),
        category: "core".into(),
        command: vec![
            bin("orchestrator", "CP_ORCHESTRATOR_BIN"),
            "--dds-domain".into(),
            cfg.default_dds_domain.to_string(),
        ],
        cwd: cfg.project_root.join("src/rust"),
        env: BTreeMap::new(),
        auto_restart: false,
        depends_on: vec![],
        tags: vec!["dds".into(), "core".into(), "rust".into()],
        overridable_args: BTreeMap::from([
            (
                "dds_domain".into(),
                OverrideValue::Int(cfg.default_dds_domain as i64),
            ),
            ("enable_agent_selection".into(), OverrideValue::Bool(false)),
        ]),
    }
}

fn studio_node(cfg: &PanelConfig) -> ServiceDefinition {
    ServiceDefinition {
        name: "studio_node".into(),
        display_name: "Studio Node".into(),
        description: "No administrativo do Studio (HTTP :4317 + presenca DDS).".into(),
        category: "support".into(),
        command: vec![bin("studio-noded", "CP_STUDIO_NODE_BIN")],
        cwd: cfg.project_root.join("src/rust"),
        env: BTreeMap::new(),
        auto_restart: false,
        depends_on: vec![],
        tags: vec!["studio".into(), "support".into()],
        overridable_args: BTreeMap::new(),
    }
}

fn llama_server(cfg: &PanelConfig) -> ServiceDefinition {
    let model = std::env::var("LLAMA_MODEL_PATH").unwrap_or_else(|_| {
        cfg.project_root
            .join("models/Qwen3.5-0.8B-Q4_K_M.gguf")
            .display()
            .to_string()
    });
    ServiceDefinition {
        name: "llama_server".into(),
        display_name: "Llama Server".into(),
        description: "Servidor llama.cpp local (binario C++).".into(),
        category: "llm_backend".into(),
        command: vec![
            bin("llama-server", "LLAMA_SERVER_BIN"),
            "-m".into(),
            model,
            "--port".into(),
            "8080".into(),
            "-c".into(),
            "4096".into(),
            "-ngl".into(),
            "99".into(),
        ],
        cwd: cfg.project_root.join("src/llama_cpp"),
        env: BTreeMap::new(),
        auto_restart: false,
        depends_on: vec![],
        tags: vec!["llm".into(), "cpp".into(), "gpu".into()],
        overridable_args: BTreeMap::new(),
    }
}

/// Um agente Rust (`agent_<id>`), equivalente a `_agent_service` atualizado.
pub fn agent_service(
    cfg: &PanelConfig,
    agent_id: &str,
    engine: &str,
    slots: i64,
) -> ServiceDefinition {
    ServiceDefinition {
        name: format!("agent_{agent_id}"),
        display_name: format!("Agent {agent_id}"),
        description: format!("Agente Rust (engine={engine}, slots={slots})."),
        category: "agent".into(),
        command: vec![
            bin("agent", "CP_AGENT_BIN"),
            "--engine".into(),
            engine.into(),
            "--dds-domain".into(),
            cfg.default_dds_domain.to_string(),
            "--agent-id".into(),
            agent_id.into(),
            "--slots".into(),
            slots.to_string(),
        ],
        cwd: cfg.project_root.join("src/rust"),
        env: BTreeMap::new(),
        auto_restart: false,
        depends_on: vec!["orchestrator".into()],
        tags: vec!["dds".into(), "agent".into(), "rust".into()],
        overridable_args: BTreeMap::from([
            (
                "dds_domain".into(),
                OverrideValue::Int(cfg.default_dds_domain as i64),
            ),
            ("slots".into(), OverrideValue::Int(slots)),
        ]),
    }
}

/// Catálogo base de serviços singleton.
pub fn get_service_catalog(cfg: &PanelConfig) -> BTreeMap<String, ServiceDefinition> {
    let mut map = BTreeMap::new();
    for svc in [orchestrator(cfg), studio_node(cfg), llama_server(cfg)] {
        map.insert(svc.name.clone(), svc);
    }
    map
}

/// Gera `count` agentes numerados `agent-<host>-N` (paridade com Python).
pub fn get_agent_services(
    cfg: &PanelConfig,
    count: usize,
    engine: &str,
    slots: i64,
) -> BTreeMap<String, ServiceDefinition> {
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".into());
    let mut map = BTreeMap::new();
    for i in 1..=count {
        let svc = agent_service(cfg, &format!("agent-{host}-{i}"), engine, slots);
        map.insert(svc.name.clone(), svc);
    }
    map
}
