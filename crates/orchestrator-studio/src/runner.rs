//! Runner local do llama-server (tela 3.4, mockup Stitch): spawn DIRETO de
//! subprocesso (sem systemd — não há unit/cgroup; o mockup cita systemd, mas
//! o Studio sobe o processo ele mesmo), stdout/stderr capturados de verdade,
//! `/health` + `/v1/models` + prova de geração reais e parada por SIGTERM com
//! escalada para SIGKILL.
//!
//! Só dados reais: PID/saída/exit do filho, latências medidas, VRAM lida do
//! hwmon AMD quando existe (ausente = "indisponível", nunca 0%), GPU por
//! detecção DRI, SHA-256 do GGUF calculado em worker. Sem GUID/build
//! inventados: a linha de build vem do `--version` real (ou "—").

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use crate::inference::{chat_completion, list_models, ChatRequest, Message, Role};
use crate::launch::StepResult;
use crate::models::{quick_inventory, ModelArtifact};

/// Teto do console (linhas; mais antigas descartadas).
pub const CONSOLE_CAP: usize = 500;
/// Espera máxima pelo `/health` (llama carrega GBs antes de servir).
pub const HEALTH_TIMEOUT_SECS: u64 = 180;
/// Cortesia do SIGTERM antes do SIGKILL.
pub const STOP_GRACE_SECS: u64 = 5;
/// Prova de geração: determinística e curta.
pub const PROOF_PROMPT: &str = "Responda apenas: INFERENCIA_OK";
/// As etapas canônicas são sempre 5 (validar, spawn, health, models, prova).
pub const TOTAL_STEPS: usize = 5;

/// Presets de perfil operacional (mockup 3.4) — valores aplicados de verdade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// Ctx 32k · batch 512 · 8 threads · CPU (seguro; usuário sobe o offload).
    Precisao,
    /// Ctx 8k · batch 1024 · 8 threads · CPU.
    Latencia,
    /// Ctx 32k · batch 512 · 8 threads · CPU.
    Agente,
}

/// Parâmetros de spawn: viram `argv` do llama-server sem reinterpretação.
#[derive(Debug, Clone)]
pub struct RunnerParams {
    pub exe: String,
    pub model: String,
    pub port: u16,
    pub ctx: u32,
    pub gpu_layers: u32,
    pub batch: u32,
    pub threads: u32,
    pub bind: String,
    /// Espera máxima pelo `/health` (s; padrão [`HEALTH_TIMEOUT_SECS`]).
    pub health_timeout_secs: u64,
}

impl RunnerParams {
    /// Padrões do mockup 3.4 (bind em loopback por segurança — 0.0.0.0 expõe
    /// inferência sem token à LAN; o aviso aparece ao escolher 0.0.0.0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            exe: detect_exe().unwrap_or_default(),
            model: String::new(),
            port: 8081,
            ctx: 32768,
            gpu_layers: 0,
            batch: 512,
            threads: 8,
            bind: String::from("127.0.0.1"),
            health_timeout_secs: HEALTH_TIMEOUT_SECS,
        }
    }

    /// `argv` real do spawn (llama-server).
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        vec![
            String::from("-m"),
            self.model.clone(),
            String::from("--port"),
            self.port.to_string(),
            String::from("--ctx-size"),
            self.ctx.to_string(),
            String::from("--n-gpu-layers"),
            self.gpu_layers.to_string(),
            String::from("--batch-size"),
            self.batch.to_string(),
            String::from("--threads"),
            self.threads.to_string(),
            String::from("--host"),
            self.bind.clone(),
        ]
    }

    /// Linha de comando exibida/copiável (== o que será executado).
    #[must_use]
    pub fn cli_preview(&self) -> String {
        format!("{} {}", self.exe, self.args().join(" "))
    }

    /// Base HTTP para as sondas (bind 0.0.0.0 sonda via loopback).
    #[must_use]
    pub fn base_url(&self) -> String {
        let host = if self.bind.trim() == "0.0.0.0" {
            "127.0.0.1"
        } else {
            self.bind.trim()
        };
        format!("http://{host}:{}", self.port)
    }
}

impl Default for RunnerParams {
    fn default() -> Self {
        Self::new()
    }
}

/// Estado do subprocesso (sempre observável, nunca presumido).
#[derive(Debug, Clone)]
pub enum RunnerStatus {
    /// Nada rodando (ou nunca rodou).
    Stopped,
    /// Etapas ①–⑤ em curso.
    Starting,
    /// Filho vivo: PID + início + último `/health` (ok?, ms).
    Running {
        pid: u32,
        since: std::time::Instant,
        healthy: Option<(bool, u64)>,
    },
    /// SIGTERM enviado, aguardando a saída (escalada em [`STOP_GRACE_SECS`]).
    Stopping { pid: u32, since: std::time::Instant },
    /// Falha terminal (texto da etapa que falhou).
    Failed { detail: String },
}

enum RunnerMsg {
    Line(String),
    Step(StepResult),
    Spawned { child: Child, pid: u32 },
    Finished,
    Health(bool, u64),
    ShaDone { path: String, hex: String },
    BuildDone { exe: String, line: String },
}

#[cfg(test)]
pub(crate) use crate::runner_probe::truncate;
/// Localiza um `llama-server` executável: caminhos conhecidos + `PATH`.
pub use crate::runner_probe::{detect_exe, fmt_gb, fmt_uptime, health_once, probe_gpu};
use crate::runner_probe::{is_executable, probe_build};

pub fn real_spawner(params: &RunnerParams) -> std::io::Result<std::process::Child> {
    Command::new(&params.exe)
        .args(params.args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

pub struct RunnerState {
    pub params: RunnerParams,
    /// Último preset aplicado intacto (`None` após edição manual).
    pub active_preset: Option<Preset>,
    pub status: RunnerStatus,
    pub steps: Vec<StepResult>,
    pub console: VecDeque<String>,
    pub autoscroll: bool,
    pub error: String,
    pub models: Vec<ModelArtifact>,
    pub models_dir: String,
    pub models_error: String,
    /// Caminho com SHA calculado (ou em cálculo) + hex.
    pub sha_for: String,
    pub sha_hex: Option<String>,
    pub sha_busy: bool,
    /// Linha de build (`--version` real) + exe que a gerou.
    pub build_for: String,
    pub build_line: String,
    pub build_busy: bool,
    pub gpu_label: Option<String>,
    pub vram: Option<(u64, u64)>,
    child: Option<Child>,
    receiver: Option<mpsc::Receiver<RunnerMsg>>,
    sender: Option<mpsc::Sender<RunnerMsg>>,
    autostart: bool,
    cancel_current: Arc<AtomicBool>,
    sha_stop: Arc<AtomicBool>,
    last_vram_poll: Option<std::time::Instant>,
    last_health_poll: Option<std::time::Instant>,
}

impl Default for RunnerState {
    fn default() -> Self {
        Self::new()
    }
}

mod runner_control;
mod runner_exec;
mod runner_prepare;

use runner_exec::run_steps;

#[cfg(test)]
mod tests;
