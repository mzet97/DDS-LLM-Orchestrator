//! Processo gerenciado (paridade com `process_manager.py::ManagedProcess`).

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("processo ja esta rodando")]
    AlreadyRunning,
    #[error("falha ao iniciar processo: {0}")]
    Spawn(#[from] std::io::Error),
}

/// Estado serializável de um processo gerenciado.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessStatus {
    pub name: String,
    pub running: bool,
    pub status: String,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub uptime_seconds: f64,
    pub log_file: String,
}

/// Subprocesso dono de um log em arquivo, com start/stop/restart idempotentes.
pub struct ManagedProcess {
    name: String,
    command: Vec<String>,
    cwd: PathBuf,
    env: BTreeMap<String, String>,
    log_file: PathBuf,
    child: Option<Child>,
    started_at: Option<Instant>,
    exit_code: Option<i32>,
}

impl ManagedProcess {
    pub fn new(
        name: String,
        command: Vec<String>,
        cwd: PathBuf,
        env: BTreeMap<String, String>,
        log_dir: &std::path::Path,
    ) -> Self {
        Self {
            log_file: log_dir.join(format!("{name}.log")),
            name,
            command,
            cwd,
            env,
            child: None,
            started_at: None,
            exit_code: None,
        }
    }

    pub fn is_running(&mut self) -> bool {
        self.refresh();
        self.child.is_some()
    }

    fn refresh(&mut self) {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.exit_code = status.code();
                    self.child = None;
                }
                Ok(None) => {}
                Err(_) => {
                    self.child = None;
                }
            }
        }
    }

    pub fn status(&mut self) -> ProcessStatus {
        self.refresh();
        let running = self.child.is_some();
        ProcessStatus {
            name: self.name.clone(),
            running,
            status: if running {
                "running".into()
            } else if self.started_at.is_some() {
                "stopped".into()
            } else {
                "not_started".into()
            },
            pid: self.child.as_ref().map(|c| c.id()),
            exit_code: self.exit_code,
            uptime_seconds: if running {
                self.started_at
                    .map(|t| t.elapsed().as_secs_f64())
                    .unwrap_or(0.0)
            } else {
                0.0
            },
            log_file: self.log_file.display().to_string(),
        }
    }

    /// Inicia o processo. Erro se já estiver rodando (paridade com o Python).
    pub fn start(&mut self) -> Result<(), ProcessError> {
        if self.is_running() {
            return Err(ProcessError::AlreadyRunning);
        }
        if let Some(parent) = self.log_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)?;
        let log_err = log.try_clone()?;
        let (program, args) = self.command.split_first().ok_or_else(|| {
            ProcessError::Spawn(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "comando vazio",
            ))
        })?;
        let child = Command::new(program)
            .args(args)
            .current_dir(&self.cwd)
            .envs(&self.env)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()?;
        self.child = Some(child);
        self.started_at = Some(Instant::now());
        self.exit_code = None;
        Ok(())
    }

    /// Para o processo (SIGTERM via kill da std; idempotente).
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        self.exit_code = status.code();
                        break;
                    }
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    _ => break,
                }
            }
        }
    }

    pub fn restart(&mut self) -> Result<(), ProcessError> {
        self.stop();
        self.start()
    }

    /// Últimas `lines` linhas do log.
    pub fn tail(&self, lines: usize) -> Vec<String> {
        let mut content = String::new();
        if File::open(&self.log_file)
            .map(|mut f| f.read_to_string(&mut content))
            .is_err()
        {
            return vec![];
        }
        let all: Vec<&str> = content.lines().collect();
        let start = all.len().saturating_sub(lines);
        all[start..].iter().map(|s| s.to_string()).collect()
    }

    pub fn clear_logs(&self) {
        let _ = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.log_file);
    }
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        self.stop();
    }
}
