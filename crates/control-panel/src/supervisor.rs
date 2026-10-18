//! Supervisor local: dono dos processos gerenciados (paridade com `supervisor.py`).

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::{json, Value};
use thiserror::Error;

use crate::catalog::{get_agent_services, OverrideValue, ServiceDefinition};
use crate::config::PanelConfig;
use crate::presets::{get_preset_by_id, resolve_preset_services};
use crate::process::{ManagedProcess, ProcessStatus};

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("servico nao encontrado no catalogo: {0}")]
    UnknownService(String),
    #[error("preset desconhecido: {0}")]
    UnknownPreset(String),
    #[error("processo nao encontrado: {0}")]
    UnknownProcess(String),
    #[error("{0}")]
    Process(#[from] crate::process::ProcessError),
}

/// Registro thread-safe de processos (equivalente ao `Supervisor` Python).
pub struct Supervisor {
    cfg: PanelConfig,
    processes: Mutex<BTreeMap<String, ManagedProcess>>,
}

impl Supervisor {
    pub fn new(cfg: PanelConfig) -> Self {
        Self {
            processes: Mutex::new(BTreeMap::new()),
            cfg,
        }
    }

    pub fn config(&self) -> &PanelConfig {
        &self.cfg
    }

    /// Inicia (ou devolve o status de) um serviço do catálogo.
    pub fn start_service(
        &self,
        definition: &ServiceDefinition,
        overrides: &BTreeMap<String, OverrideValue>,
    ) -> Result<ProcessStatus, SupervisorError> {
        let mut map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(proc) = map.get_mut(&definition.name) {
            if proc.is_running() {
                return Ok(proc.status());
            }
        }
        let command = definition.build_command(overrides);
        let mut env: BTreeMap<String, String> = std::env::vars().collect();
        env.extend(definition.env.clone());
        let mut proc = ManagedProcess::new(
            definition.name.clone(),
            command,
            definition.cwd.clone(),
            env,
            &self.cfg.log_dir,
        );
        proc.start()?;
        let status = proc.status();
        map.insert(definition.name.clone(), proc);
        Ok(status)
    }

    pub fn stop_service(&self, name: &str) -> Result<ProcessStatus, SupervisorError> {
        let mut map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        let proc = map
            .get_mut(name)
            .ok_or_else(|| SupervisorError::UnknownProcess(name.into()))?;
        proc.stop();
        Ok(proc.status())
    }

    pub fn restart_service(&self, name: &str) -> Result<ProcessStatus, SupervisorError> {
        let mut map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        let proc = map
            .get_mut(name)
            .ok_or_else(|| SupervisorError::UnknownProcess(name.into()))?;
        proc.restart()?;
        Ok(proc.status())
    }

    pub fn status(&self, name: &str) -> ProcessStatus {
        let mut map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        match map.get_mut(name) {
            Some(proc) => proc.status(),
            None => ProcessStatus {
                name: name.into(),
                running: false,
                status: "not_started".into(),
                pid: None,
                exit_code: None,
                uptime_seconds: 0.0,
                log_file: self
                    .cfg
                    .log_dir
                    .join(format!("{name}.log"))
                    .display()
                    .to_string(),
            },
        }
    }

    pub fn list_status(&self) -> Vec<ProcessStatus> {
        let mut map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        map.values_mut().map(|proc| proc.status()).collect()
    }

    pub fn get_logs(&self, name: &str, lines: usize) -> Vec<String> {
        let map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        map.get(name).map(|p| p.tail(lines)).unwrap_or_default()
    }

    pub fn clear_logs(&self, name: &str) -> Result<(), SupervisorError> {
        let map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        let proc = map
            .get(name)
            .ok_or_else(|| SupervisorError::UnknownProcess(name.into()))?;
        proc.clear_logs();
        Ok(())
    }

    pub fn stop_all(&self) {
        let mut map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
        for proc in map.values_mut() {
            proc.stop();
        }
    }

    /// Inicia `count` agentes dinâmicos (POST /agents/start do Python).
    pub fn start_agents(
        &self,
        count: usize,
        engine: &str,
        slots: i64,
    ) -> Result<Vec<ProcessStatus>, SupervisorError> {
        let agents = get_agent_services(&self.cfg, count, engine, slots);
        let mut out = Vec::new();
        for svc in agents.values() {
            out.push(self.start_service(svc, &BTreeMap::new())?);
        }
        Ok(out)
    }

    pub fn stop_all_agents(&self) -> Vec<String> {
        let names: Vec<String> = {
            let map = self.processes.lock().unwrap_or_else(|p| p.into_inner());
            map.keys()
                .filter(|n| n.starts_with("agent_"))
                .cloned()
                .collect()
        };
        for name in &names {
            let _ = self.stop_service(name);
        }
        names
    }

    /// Aplica um preset na ordem core → support → llm_backend → agent.
    pub fn apply_preset(&self, preset_id: &str) -> Result<Vec<ProcessStatus>, SupervisorError> {
        let preset = get_preset_by_id(preset_id)
            .ok_or_else(|| SupervisorError::UnknownPreset(preset_id.into()))?;
        let services =
            resolve_preset_services(&self.cfg, &preset).map_err(SupervisorError::UnknownService)?;
        let order = |c: &str| match c {
            "core" => 0,
            "support" => 1,
            "llm_backend" => 2,
            "agent" => 3,
            _ => 99,
        };
        let mut defs: Vec<&ServiceDefinition> = services.values().collect();
        defs.sort_by_key(|d| order(&d.category));
        let mut out = Vec::new();
        for def in defs {
            out.push(self.start_service(def, &BTreeMap::new())?);
        }
        Ok(out)
    }

    /// Resumo do dashboard (metrics.py): serviços + contagens.
    pub fn dashboard_summary(&self) -> Value {
        let statuses = self.list_status();
        let running = statuses.iter().filter(|s| s.running).count();
        json!({
            "supervisor_online": true,
            "running_services": running,
            "total_managed_services": statuses.len(),
            "services_total": statuses.len(),
            "services_running": running,
            "services": statuses,
            "presets": crate::presets::get_presets()
                .iter()
                .map(|p| json!({"id": p.id, "name": p.name, "description": p.description}))
                .collect::<Vec<_>>(),
        })
    }
}
