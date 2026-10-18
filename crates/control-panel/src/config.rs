//! Configuração do painel (paridade com `src/control_panel/config.py`).

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Chaves de ambiente que nunca são serializadas (config.py:80).
pub fn sensitive_env_keys() -> BTreeSet<&'static str> {
    [
        "OPENROUTER_API_KEY",
        "DATABASE_URL",
        "AWS_SECRET_ACCESS_KEY",
    ]
    .into_iter()
    .collect()
}

/// Chaves de inventário sensíveis por nome exato (config.py:127).
pub fn sensitive_inventory_keys() -> BTreeSet<&'static str> {
    [
        "ansible_ssh_pass",
        "ansible_password",
        "ansible_become_pass",
        "ansible_ssh_private_key_file",
    ]
    .into_iter()
    .collect()
}

/// Verdadeiro se a chave de inventário é sensível (config.py + vm_inventory.py:_is_sensitive_key).
pub fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    sensitive_inventory_keys().contains(lower.as_str())
        || lower.contains("pass")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("private_key")
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_int(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Caminhos e portas do painel, resolvidos do ambiente (prefixo `CP_`).
#[derive(Debug, Clone)]
pub struct PanelConfig {
    pub project_root: PathBuf,
    pub supervisor_host: String,
    pub supervisor_port: u16,
    pub log_dir: PathBuf,
    pub automation_root: PathBuf,
    pub ansible_inventory: PathBuf,
    pub default_dds_domain: i32,
    pub user_config_file: PathBuf,
}

impl PanelConfig {
    /// Resolve a configuração do ambiente, com defaults do painel Python.
    pub fn from_env() -> Self {
        let project_root = std::env::var("CP_PROJECT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let automation_root = project_root.join("src/automation");
        Self {
            ansible_inventory: std::env::var("CP_ANSIBLE_INVENTORY")
                .map(PathBuf::from)
                .unwrap_or_else(|_| automation_root.join("inventory.ini")),
            automation_root,
            project_root,
            supervisor_host: env_or("CP_SUPERVISOR_HOST", "127.0.0.1"),
            supervisor_port: env_int("CP_SUPERVISOR_PORT", 8150),
            log_dir: std::env::var("CP_LOG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    std::env::var("XDG_DATA_HOME")
                        .map(|x| PathBuf::from(x).join("dds-control-panel"))
                        .unwrap_or_else(|_| PathBuf::from("/tmp/dds-control-panel"))
                }),
            user_config_file: std::env::var("CP_USER_CONFIG")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    std::env::var("XDG_CONFIG_HOME")
                        .map(|x| PathBuf::from(x).join("dds-control-panel/config.json"))
                        .unwrap_or_else(|_| PathBuf::from("/tmp/dds-control-panel-config.json"))
                }),
            default_dds_domain: std::env::var("DDS_DOMAIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        }
    }
}
