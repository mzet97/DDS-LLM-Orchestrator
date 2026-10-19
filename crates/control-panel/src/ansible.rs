//! Runner Ansible seguro (paridade com `ansible_runner.py`/`vm_manager.py`):
//! comando como lista, playbook dentro da raiz, allowlist, redação, 1 run por vez.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use thiserror::Error;

use crate::inventory::redact;

#[derive(Debug, Error)]
pub enum AnsibleError {
    #[error("acao desconhecida: {0}")]
    UnknownAction(String),
    #[error("playbook fora da raiz de automacao: {0}")]
    PlaybookEscape(String),
    #[error("playbook nao encontrado: {0}")]
    PlaybookMissing(String),
    #[error("limit invalido: {0}")]
    InvalidLimit(String),
    #[error("ja existe uma execucao Ansible ativa")]
    Busy,
    #[error("falha ao executar ansible: {0}")]
    Spawn(#[from] std::io::Error),
}

/// Ação permitida no painel (config.py:95-124).
#[derive(Debug, Clone, Serialize)]
pub struct AnsibleAction {
    pub id: String,
    pub label: String,
    pub playbook: String,
    pub description: String,
    pub dangerous: bool,
}

/// Allowlist de ações (espelho de ANSIBLE_PLAYBOOK_ACTIONS).
pub fn list_actions() -> Vec<AnsibleAction> {
    vec![
        AnsibleAction {
            id: "status".into(),
            label: "Checar status".into(),
            playbook: "status.yml".into(),
            description: "Lista processos ativos em cada host.".into(),
            dangerous: false,
        },
        AnsibleAction {
            id: "deploy".into(),
            label: "Deploy cluster".into(),
            playbook: "deploy-cluster.yml".into(),
            description: "Deploy completo do cluster.".into(),
            dangerous: false,
        },
        AnsibleAction {
            id: "deploy-v2".into(),
            label: "Deploy cluster v2".into(),
            playbook: "deploy-cluster-v2.yml".into(),
            description: "Deploy via playbook v2.".into(),
            dangerous: false,
        },
        AnsibleAction {
            id: "stop".into(),
            label: "Parar cluster".into(),
            playbook: "stop-cluster-v2.yml".into(),
            description: "Para os servicos do cluster.".into(),
            dangerous: true,
        },
    ]
}

/// Registro de uma execução (AnsibleRun do Python, forma pública redigida).
#[derive(Debug, Clone, Serialize)]
pub struct AnsibleRun {
    pub id: String,
    pub action: String,
    pub command: Vec<String>,
    pub status: String,
    pub return_code: Option<i32>,
    pub lines: Vec<String>,
    pub error: Option<String>,
}

/// Runner com no máximo 1 execução ativa (VM_MAX_CONCURRENT_RUNS=1).
pub struct AnsibleRunner {
    pub(crate) automation_root: PathBuf,
    pub(crate) inventory: PathBuf,
    pub(crate) runs: Mutex<BTreeMap<String, AnsibleRun>>,
    pub(crate) active: Mutex<bool>,
    pub(crate) active_child: Mutex<Option<std::process::Child>>,
    pub(crate) sequence: AtomicU64,
}

impl AnsibleRunner {
    pub fn new(automation_root: PathBuf, inventory: PathBuf) -> Self {
        Self {
            automation_root,
            inventory,
            runs: Mutex::new(BTreeMap::new()),
            active: Mutex::new(false),
            active_child: Mutex::new(None),
            sequence: AtomicU64::new(1),
        }
    }

    /// Resolve o playbook garantindo que está dentro de `automation_root`.
    pub fn resolve_playbook(&self, playbook: &str) -> Result<PathBuf, AnsibleError> {
        let candidate = Path::new(playbook);
        if candidate.is_absolute()
            || candidate
                .components()
                .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(AnsibleError::PlaybookEscape(playbook.into()));
        }
        let resolved = self.automation_root.join(candidate);
        if !resolved.exists() {
            return Err(AnsibleError::PlaybookMissing(playbook.into()));
        }
        Ok(resolved)
    }

    /// Monta o comando como lista (nunca shell) — `ansible-playbook -i <inv> <playbook> [--limit <limit>]`.
    pub fn build_command(
        &self,
        playbook: &Path,
        limit: Option<&str>,
        allowed_limits: &BTreeSet<String>,
    ) -> Result<Vec<String>, AnsibleError> {
        let mut cmd = vec![
            "ansible-playbook".to_string(),
            "-i".to_string(),
            self.inventory.display().to_string(),
            playbook.display().to_string(),
        ];
        if let Some(limit) = limit {
            if !allowed_limits.contains(limit) {
                return Err(AnsibleError::InvalidLimit(limit.into()));
            }
            cmd.push("--limit".into());
            cmd.push(limit.into());
        }
        Ok(cmd)
    }

    /// Executa a ação sincronamente, redigindo segredos da saída.
    pub fn start(
        &self,
        action_id: &str,
        limit: Option<&str>,
        inventory_secrets: &BTreeSet<String>,
        allowed_limits: &BTreeSet<String>,
    ) -> Result<AnsibleRun, AnsibleError> {
        let action = list_actions()
            .into_iter()
            .find(|a| a.id == action_id)
            .ok_or_else(|| AnsibleError::UnknownAction(action_id.into()))?;
        {
            let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
            if *active {
                return Err(AnsibleError::Busy);
            }
            *active = true;
        }
        let result = self.execute(&action, limit, inventory_secrets, allowed_limits);
        *self.active.lock().unwrap_or_else(|p| p.into_inner()) = false;
        let run = result?;
        self.runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(run.id.clone(), run.clone());
        Ok(run)
    }

    fn execute(
        &self,
        action: &AnsibleAction,
        limit: Option<&str>,
        secrets: &BTreeSet<String>,
        allowed_limits: &BTreeSet<String>,
    ) -> Result<AnsibleRun, AnsibleError> {
        let playbook = self.resolve_playbook(&action.playbook)?;
        let command = self.build_command(&playbook, limit, allowed_limits)?;
        let id = format!("run-{}", self.sequence.fetch_add(1, Ordering::Relaxed));
        let Some((program, args)) = command.split_first() else {
            return Err(AnsibleError::UnknownAction("comando vazio".into()));
        };
        let output = std::process::Command::new(program)
            .args(args)
            .current_dir(&self.automation_root)
            .output()?;
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let lines: Vec<String> = redact(&text, secrets)
            .lines()
            .map(|l| l.to_string())
            .collect();
        Ok(AnsibleRun {
            id,
            action: action.id.clone(),
            command,
            status: if output.status.success() {
                "finished".into()
            } else {
                "failed".into()
            },
            return_code: output.status.code(),
            lines,
            error: None,
        })
    }

    pub fn list_runs(&self) -> Vec<AnsibleRun> {
        self.runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub fn active_count(&self) -> usize {
        usize::from(*self.active.lock().unwrap_or_else(|p| p.into_inner()))
    }
}
