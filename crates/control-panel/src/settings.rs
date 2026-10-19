//! Configurações do painel (paridade com `web/routes/settings.py`):
//! GET com segredos mascarados e POST com allowlist persistido em arquivo.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::config::PanelConfig;

const ALLOWED_KEYS: [&str; 3] = ["theme", "default_preset", "refresh_interval_ms"];

/// Carrega a configuração do usuário ({} se ausente/ inválida).
pub fn load_user_config(cfg: &PanelConfig) -> Value {
    std::fs::read_to_string(&cfg.user_config_file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

/// Persiste apenas as chaves permitidas; devolve as chaves salvas.
pub fn save_user_config(
    cfg: &PanelConfig,
    payload: &Map<String, Value>,
) -> Result<Vec<String>, String> {
    let mut current = match load_user_config(cfg) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let mut saved = Vec::new();
    for (key, value) in payload {
        if ALLOWED_KEYS.contains(&key.as_str()) {
            current.insert(key.clone(), value.clone());
            saved.push(key.clone());
        }
    }
    if let Some(parent) = cfg.user_config_file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(current)).map_err(|e| e.to_string())?;
    std::fs::write(&cfg.user_config_file, text).map_err(|e| e.to_string())?;
    Ok(saved)
}

/// Visão de settings (segredos nunca incluídos).
pub fn settings_view(cfg: &PanelConfig) -> Value {
    json!({
        "panel": {"host": cfg.supervisor_host, "port": cfg.supervisor_port},
        "supervisor": {"host": cfg.supervisor_host, "port": cfg.supervisor_port},
        "dds": {"domain": cfg.default_dds_domain},
        "paths": {
            "project_root": cfg.project_root.display().to_string(),
            "ansible_inventory": cfg.ansible_inventory.display().to_string(),
            "log_dir": cfg.log_dir.display().to_string(),
        },
        "model": {
            "llama_model_path": std::env::var("LLAMA_MODEL_PATH").unwrap_or_else(|_| cfg.project_root.join("models/Qwen3.5-0.8B-Q4_K_M.gguf").display().to_string()),
            "llama_server_bin": std::env::var("LLAMA_SERVER_BIN").unwrap_or_else(|_| cfg.project_root.join("src/llama_cpp/build/bin/llama-server").display().to_string()),
        },
        "user_config": load_user_config(cfg),
    })
}

/// Máscara de valores de um mapa por chaves sensíveis (uso em views).
pub fn mask_secrets(map: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                if crate::config::is_sensitive_key(k) {
                    "******".into()
                } else {
                    v.clone()
                },
            )
        })
        .collect()
}
