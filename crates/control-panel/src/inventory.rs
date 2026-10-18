//! Parser de inventário Ansible + redação de segredos.
//! Paridade com `vm_inventory.py` (grupos, hosts, :children, redação).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::is_sensitive_key;

/// Host do inventário (VMHost do Python).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VmHost {
    pub name: String,
    pub address: String,
    pub groups: Vec<String>,
    pub ansible_user: Option<String>,
    pub connection: Option<String>,
    /// Variáveis brutas (uso interno; serialização pública usa `to_public`).
    #[serde(skip)]
    pub variables: BTreeMap<String, String>,
}

impl VmHost {
    /// Visão pública com variáveis sensíveis redigidas (to_dict do Python).
    pub fn to_public(&self) -> serde_json::Value {
        let variables: BTreeMap<String, String> = self
            .variables
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    if is_sensitive_key(k) {
                        "******".into()
                    } else {
                        v.clone()
                    },
                )
            })
            .collect();
        serde_json::json!({
            "name": self.name,
            "address": self.address,
            "groups": self.groups,
            "ansible_user": self.ansible_user,
            "connection": self.connection,
            "variables": variables,
        })
    }
}

/// Inventário completo (VMInventory do Python).
#[derive(Debug, Clone, PartialEq)]
pub struct VmInventory {
    pub hosts: Vec<VmHost>,
    pub groups: BTreeMap<String, Vec<String>>,
    pub source: PathBuf,
}

impl VmInventory {
    pub fn host_names(&self) -> BTreeSet<String> {
        self.hosts.iter().map(|h| h.name.clone()).collect()
    }

    pub fn group_names(&self) -> BTreeSet<String> {
        self.groups.keys().cloned().collect()
    }

    /// Valores aceitos em `--limit`: hosts ∪ grupos.
    pub fn all_limit_values(&self) -> BTreeSet<String> {
        self.host_names()
            .union(&self.group_names())
            .cloned()
            .collect()
    }

    /// Segredos conhecidos (valores de variáveis sensíveis) para redação de logs.
    pub fn secrets(&self) -> BTreeSet<String> {
        self.hosts
            .iter()
            .flat_map(|h| {
                h.variables
                    .iter()
                    .filter(|(k, _)| is_sensitive_key(k))
                    .map(|(_, v)| v.clone())
            })
            .collect()
    }

    pub fn to_public(&self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source.display().to_string(),
            "hosts": self.hosts.iter().map(|h| h.to_public()).collect::<Vec<_>>(),
            "groups": self.groups,
        })
    }
}

fn parse_host_line(line: &str) -> (String, BTreeMap<String, String>) {
    let mut parts = line.split_whitespace();
    let name = parts.next().unwrap_or_default().to_string();
    let mut variables = BTreeMap::new();
    for token in parts {
        if let Some((key, value)) = token.split_once('=') {
            variables.insert(
                key.trim().to_string(),
                value.trim().trim_matches(['"', '\'']).to_string(),
            );
        }
    }
    (name, variables)
}

/// Carrega o inventário; arquivo ausente ⇒ inventário vazio (paridade Python).
pub fn load_inventory(path: &Path) -> VmInventory {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => {
            return VmInventory {
                hosts: vec![],
                groups: BTreeMap::new(),
                source: path.to_path_buf(),
            }
        }
    };

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut raw_hosts: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let header = line[1..line.len() - 1].trim().to_string();
            if !header.ends_with(":vars") && !header.ends_with(":children") {
                groups.entry(header.clone()).or_default();
            }
            current = Some(header);
            continue;
        }
        let Some(group) = current.clone() else {
            continue;
        };
        if group.ends_with(":vars") {
            continue;
        }
        if let Some(parent) = group.strip_suffix(":children") {
            let child = line.split_whitespace().next().unwrap_or_default();
            if let Some(members) = groups.get(child).cloned() {
                groups
                    .entry(parent.to_string())
                    .or_default()
                    .extend(members);
            }
            continue;
        }
        let (host_name, vars) = parse_host_line(line);
        if host_name.is_empty() {
            continue;
        }
        groups.entry(group).or_default().push(host_name.clone());
        raw_hosts.entry(host_name).or_default().extend(vars);
    }

    for members in groups.values_mut() {
        members.sort();
        members.dedup();
    }

    let hosts = raw_hosts
        .into_iter()
        .map(|(name, data)| VmHost {
            address: data
                .get("ansible_host")
                .cloned()
                .unwrap_or_else(|| name.clone()),
            ansible_user: data.get("ansible_user").cloned(),
            connection: data.get("ansible_connection").cloned(),
            groups: groups
                .iter()
                .filter(|(_, members)| members.contains(&name))
                .map(|(g, _)| g.clone())
                .collect(),
            variables: data,
            name,
        })
        .collect();

    VmInventory {
        hosts,
        groups,
        source: path.to_path_buf(),
    }
}

/// Substitui segredos (≥3 chars, paridade com o Python corrigido) por `******`.
pub fn redact(text: &str, secrets: &BTreeSet<String>) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        if secret.len() >= 3 {
            out = out.replace(secret.as_str(), "******");
        }
    }
    out
}
