//! Runs assíncronas do Ansible (stream/cancel) — impl de `AnsibleRunner`.

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use crate::ansible::{list_actions, AnsibleError, AnsibleRun};
use crate::inventory::redact;

impl crate::ansible::AnsibleRunner {
    /// Inicia a ação em thread (stream/cancel via `get_run`/`cancel`).
    /// A guarda de 1 run ativa é a mesma do caminho síncrono.
    pub fn start_async(
        self: &std::sync::Arc<Self>,
        action_id: &str,
        limit: Option<&str>,
        inventory_secrets: &BTreeSet<String>,
        allowed_limits: &BTreeSet<String>,
    ) -> Result<AnsibleRun, AnsibleError> {
        let action = list_actions()
            .into_iter()
            .find(|a| a.id == action_id)
            .ok_or_else(|| AnsibleError::UnknownAction(action_id.into()))?;
        let playbook = self.resolve_playbook(&action.playbook)?;
        let command = self.build_command(&playbook, limit, allowed_limits)?;
        {
            let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
            if *active {
                return Err(AnsibleError::Busy);
            }
            *active = true;
        }
        let id = format!("run-{}", self.sequence.fetch_add(1, Ordering::Relaxed));
        let run = AnsibleRun {
            id: id.clone(),
            action: action.id.clone(),
            command: command.clone(),
            status: "running".into(),
            return_code: None,
            lines: vec![],
            error: None,
        };
        self.runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), run.clone());

        let runner = self.clone();
        let secrets = inventory_secrets.clone();
        std::thread::spawn(move || {
            runner.execute_streaming(&id, &command, &secrets);
            *runner.active.lock().unwrap_or_else(|p| p.into_inner()) = false;
        });
        Ok(run)
    }

    fn execute_streaming(&self, id: &str, command: &[String], secrets: &BTreeSet<String>) {
        use std::io::{BufRead, BufReader};
        let Some((program, args)) = command.split_first() else {
            self.finish_run(id, "failed", None, Some("comando vazio".into()));
            return;
        };
        let spawned = std::process::Command::new(program)
            .args(args)
            .current_dir(&self.automation_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                self.finish_run(id, "failed", None, Some(e.to_string()));
                return;
            }
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        *self.active_child.lock().unwrap_or_else(|p| p.into_inner()) = Some(child);
        // Lê stdout/stderr em threads e anexa linhas redigidas ao registro.
        // (O registro é a fonte do stream SSE: polling de linhas novas.)
        let streams: Vec<Box<dyn std::io::Read + Send>> = [
            stdout.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            stderr.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        .collect();
        let handles: Vec<_> = streams
            .into_iter()
            .map(|stream| {
                // Linhas são coletadas ao fim de cada stream para manter a
                // ordem simples e determinística por fonte.
                std::thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    reader
                        .lines()
                        .map_while(Result::ok)
                        .collect::<Vec<String>>()
                })
            })
            .collect();
        let mut all_lines: Vec<String> = Vec::new();
        for handle in handles {
            if let Ok(lines) = handle.join() {
                all_lines.extend(lines);
            }
        }
        let status = {
            let mut guard = self.active_child.lock().unwrap_or_else(|p| p.into_inner());
            match guard.as_mut().map(|c| c.wait()) {
                Some(Ok(status)) => Some(status),
                _ => None,
            }
        };
        if let Ok(mut guard) = self.active_child.try_lock() {
            *guard = None;
        }
        let cancelled = self
            .runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .map(|r| r.status == "cancelling")
            .unwrap_or(false);
        let mut lines: Vec<String> = all_lines.iter().map(|l| redact(l, secrets)).collect();
        if let Some(run) = self
            .runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(id)
        {
            run.lines.append(&mut lines);
            run.return_code = status.and_then(|s| s.code());
            run.status = if cancelled {
                "cancelled".into()
            } else if status.map(|s| s.success()).unwrap_or(false) {
                "finished".into()
            } else {
                "failed".into()
            };
        }
    }

    fn finish_run(&self, id: &str, status: &str, code: Option<i32>, error: Option<String>) {
        if let Some(run) = self
            .runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(id)
        {
            run.status = status.into();
            run.return_code = code;
            run.error = error;
        }
    }

    /// Cancela a run ativa (mata o processo filho).
    pub fn cancel(&self, id: &str) -> Result<AnsibleRun, AnsibleError> {
        {
            let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
            let run = runs
                .get_mut(id)
                .ok_or_else(|| AnsibleError::UnknownAction(id.into()))?;
            if run.status != "running" {
                return Ok(run.clone());
            }
            run.status = "cancelling".into();
        }
        if let Some(child) = self
            .active_child
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            let _ = child.kill();
        }
        self.runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(|| AnsibleError::UnknownAction(id.into()))
    }

    pub fn get_run(&self, id: &str) -> Option<AnsibleRun> {
        self.runs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
    }
}
