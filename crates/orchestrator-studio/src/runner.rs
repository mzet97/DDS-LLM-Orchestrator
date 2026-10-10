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

/// Localiza um `llama-server` executável: caminhos conhecidos + `PATH`.
#[must_use]
pub fn detect_exe() -> Option<String> {
    let mut candidates = vec![
        String::from("/usr/bin/llama-server"),
        String::from("/usr/local/bin/llama-server"),
    ];
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            candidates.push(format!("{dir}/llama-server"));
        }
    }
    candidates.into_iter().find(|exe| is_executable(exe))
}

/// Verdadeiro quando o caminho existe e tem bit de execução (unix).
fn is_executable(path: &str) -> bool {
    let candidate = std::path::Path::new(path);
    if !candidate.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        candidate
            .metadata()
            .is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Acelerador via DRI real: `(rótulo, vram_usada_B?, vram_total_B?)`.
/// `None` = sem GPU DRI (CPU) ou sem hwmon (VRAM indisponível, nunca 0%).
#[must_use]
pub fn probe_gpu() -> (Option<String>, Option<(u64, u64)>) {
    let mut label = None;
    let mut vram = None;
    let Ok(drm) = std::fs::read_dir("/sys/class/drm") else {
        return (None, None);
    };
    let mut cards: Vec<String> = drm
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("card") && !name.contains('-'))
        .collect();
    cards.sort();
    for card in cards {
        let base = format!("/sys/class/drm/{card}/device");
        let hwmon = std::fs::read_dir(format!("{base}/hwmon"))
            .ok()
            .and_then(|mut dirs| dirs.next())
            .and_then(|entry| entry.ok())
            .map(|entry| entry.path().join("name"));
        let name = hwmon
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|raw| raw.trim().to_owned())
            .filter(|raw| !raw.is_empty());
        let total = std::fs::read_to_string(format!("{base}/mem_info_vram_total"))
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok());
        let used = std::fs::read_to_string(format!("{base}/mem_info_vram_used"))
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok());
        if name.is_some() || total.is_some() {
            label = Some(format!("{} ({card})", name.as_deref().unwrap_or("gpu-dri")));
            if let (Some(used), Some(total)) = (used, total) {
                vram = Some((used, total));
            }
            break;
        }
    }
    (label, vram)
}

/// Uma sonda `/health` (ms medidos). `Ok` = 200.
pub fn health_once(base_url: &str) -> Result<u64, String> {
    let url = base_url.trim_end_matches('/');
    let started = std::time::Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|err| err.to_string())?;
    client
        .get(format!("{url}/health"))
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    Ok(started.elapsed().as_millis() as u64)
}

/// Spawner real: `argv` dos params, stdout/stderr em pipe para o console.
pub fn real_spawner(params: &RunnerParams) -> std::io::Result<std::process::Child> {
    Command::new(&params.exe)
        .args(params.args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Uptime humano ("45s", "3m 12s", "2h 05m").
#[must_use]
pub fn fmt_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Gibibytes com 1 casa ("5.4").
#[must_use]
pub fn fmt_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000_000.0)
}

/// Estado do runner: formulário + subprocesso + console ao vivo.
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

impl RunnerState {
    #[must_use]
    pub fn new() -> Self {
        let mut state = Self {
            params: RunnerParams::new(),
            active_preset: None,
            status: RunnerStatus::Stopped,
            steps: Vec::new(),
            console: VecDeque::new(),
            autoscroll: true,
            error: String::new(),
            models: Vec::new(),
            models_dir: crate::models::default_models_dir(),
            models_error: String::new(),
            sha_for: String::new(),
            sha_hex: None,
            sha_busy: false,
            build_for: String::new(),
            build_line: String::new(),
            build_busy: false,
            gpu_label: None,
            vram: None,
            child: None,
            receiver: None,
            sender: None,
            autostart: false,
            cancel_current: Arc::new(AtomicBool::new(false)),
            sha_stop: Arc::new(AtomicBool::new(false)),
            last_vram_poll: None,
            last_health_poll: None,
        };
        state.refresh_models();
        let (label, vram) = probe_gpu();
        state.gpu_label = label;
        state.vram = vram;
        state
    }

    /// Há subprocesso vivo ou transição em curso (SUBIR desabilita).
    #[must_use]
    pub fn running(&self) -> bool {
        matches!(
            self.status,
            RunnerStatus::Starting | RunnerStatus::Running { .. } | RunnerStatus::Stopping { .. }
        )
    }

    /// PID vivo, quando há.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        match &self.status {
            RunnerStatus::Running { pid, .. } | RunnerStatus::Stopping { pid, .. } => Some(*pid),
            RunnerStatus::Starting | RunnerStatus::Stopped | RunnerStatus::Failed { .. } => None,
        }
    }

    /// Relê o diretório de GGUFs (rápido: só `read_dir` + metadata).
    pub fn refresh_models(&mut self) {
        if self.models_dir.trim().is_empty() {
            self.models.clear();
            self.models_error =
                String::from("diretório vazio — defina STUDIO_MODELS_DIR ou ~/tese/models");
            return;
        }
        match quick_inventory(std::path::Path::new(&self.models_dir)) {
            Ok(list) => {
                self.models_error.clear();
                self.models = list;
            }
            Err(err) => {
                self.models.clear();
                self.models_error = err.to_string();
            }
        }
    }

    /// Aplica um preset do mockup (valores reais nos campos).
    pub fn apply_preset(&mut self, preset: Preset) {
        self.active_preset = Some(preset);
        match preset {
            Preset::Precisao => {
                self.params.ctx = 32768;
                self.params.batch = 512;
                self.params.threads = 8;
            }
            Preset::Latencia => {
                self.params.ctx = 8192;
                self.params.batch = 1024;
                self.params.threads = 8;
            }
            Preset::Agente => {
                self.params.ctx = 32768;
                self.params.batch = 256;
                self.params.threads = 8;
            }
        }
    }

    /// Valida sem efeito: executável, modelo, porta livre e números úteis.
    pub fn validate(&self) -> Result<(), String> {
        if self.params.exe.trim().is_empty() {
            return Err(String::from("informe o executável do llama-server"));
        }
        if !is_executable(self.params.exe.trim()) {
            return Err(format!(
                "{} não encontrado ou sem execução",
                self.params.exe
            ));
        }
        if self.params.model.trim().is_empty() {
            return Err(String::from("escolha o artefato GGUF"));
        }
        if !std::path::Path::new(self.params.model.trim()).is_file() {
            return Err(format!("modelo ausente: {}", self.params.model));
        }
        if self.params.port == 0 {
            return Err(String::from("porta precisa ser maior que zero"));
        }
        if self.params.ctx == 0 || self.params.batch == 0 || self.params.threads == 0 {
            return Err(String::from(
                "contexto, batch e threads precisam ser maiores que zero",
            ));
        }
        let bind = format!("{}:{}", self.params.bind.trim(), self.params.port);
        match std::net::TcpListener::bind(&bind) {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                Err(format!("porta ocupada: {bind}"))
            }
            Err(err) => Err(format!("bind inválido ({bind}): {err}")),
        }
    }

    /// Garante a linha de build do exe atual (worker; idempotente).
    pub fn ensure_build(&mut self) {
        let exe = self.params.exe.trim().to_owned();
        if exe.is_empty() || self.build_busy || self.build_for == exe {
            return;
        }
        let Some(tx) = self.sender.clone().or_else(|| self.ensure_channel()) else {
            return;
        };
        self.build_busy = true;
        self.build_for = exe.clone();
        std::thread::spawn(move || {
            let line = probe_build(&exe);
            let _ = tx.send(RunnerMsg::BuildDone { exe, line });
        });
    }

    /// Garante o SHA-256 do modelo atual (worker; idempotente).
    pub fn ensure_sha(&mut self) {
        let model = self.params.model.trim().to_owned();
        if model.is_empty() || self.sha_busy || self.sha_for == model {
            return;
        }
        let Some(tx) = self.sender.clone().or_else(|| self.ensure_channel()) else {
            return;
        };
        self.sha_stop.store(true, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        self.sha_stop = stop.clone();
        self.sha_busy = true;
        self.sha_for = model.clone();
        self.sha_hex = None;
        std::thread::spawn(move || {
            let path = std::path::PathBuf::from(&model);
            let dir = path
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let name = path
                .file_name()
                .map(|raw| raw.to_string_lossy().into_owned())
                .unwrap_or_default();
            let hex =
                crate::models::hash_file_with_progress(&dir, &name, Some(&stop), |_, _, _| {}).ok();
            if stop.load(Ordering::Relaxed) {
                return;
            }
            if let Some(hex) = hex {
                let _ = tx.send(RunnerMsg::ShaDone { path: model, hex });
            }
        });
    }

    fn ensure_channel(&mut self) -> Option<mpsc::Sender<RunnerMsg>> {
        if self.sender.is_none() {
            let (tx, rx) = mpsc::channel();
            self.sender = Some(tx);
            self.receiver = Some(rx);
        }
        self.sender.clone()
    }

    fn push_line(&mut self, line: String) {
        if self.console.len() == CONSOLE_CAP {
            self.console.pop_front();
        }
        self.console.push_back(line);
    }

    /// Caminho real: valida e sobe com o spawner de produção.
    pub fn start(&mut self) {
        self.start_with(real_spawner);
    }

    /// Como [`start`](Self::start), com spawner injetável (testes sobem
    /// `/bin/echo`/`sleep` de verdade: PID/saída/exit reais, sem llama).
    pub fn start_with(
        &mut self,
        spawner: impl FnOnce(&RunnerParams) -> std::io::Result<Child> + Send + 'static,
    ) {
        if self.running() {
            return;
        }
        if let Err(detail) = self.validate() {
            self.error = detail;
            return;
        }
        self.error.clear();
        self.steps.clear();
        self.autostart = false;
        self.cancel_current = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel_current.clone();
        let Some(tx) = self.ensure_channel() else {
            return;
        };
        let params = self.params.clone();
        self.push_line(String::from("── nova execução ──"));
        self.status = RunnerStatus::Starting;
        std::thread::spawn(move || {
            run_steps(params, spawner, tx, cancel);
        });
    }

    /// SIGTERM com escalada: a UI observa a saída em [`poll`](Self::poll) e
    /// manda SIGKILL após [`STOP_GRACE_SECS`] sem efeito.
    pub fn stop(&mut self) {
        let Some(pid) = self.child.as_mut().map(|child| child.id()) else {
            return;
        };
        self.cancel_current.store(true, Ordering::Relaxed);
        self.autostart = false;
        // SAFETY: kill(2) com PID vivo e SIGTERM — sem memória compartilhada.
        let termed = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0;
        self.push_line(if termed {
            format!("runner: SIGTERM enviado ao PID {pid}")
        } else {
            format!("runner: PID {pid} já havia saído")
        });
        self.status = RunnerStatus::Stopping {
            pid,
            since: std::time::Instant::now(),
        };
    }

    /// Para e sobe de novo quando o filho sair (spawner real).
    pub fn restart(&mut self) {
        if self.child.is_none() {
            self.start();
            return;
        }
        self.stop();
        self.autostart = true;
    }

    /// Drena workers + observa o filho; chamar por frame.
    pub fn poll(&mut self) {
        while let Ok(msg) = self
            .receiver
            .as_ref()
            .map_or(Err(mpsc::TryRecvError::Empty), mpsc::Receiver::try_recv)
        {
            match msg {
                RunnerMsg::Line(line) => self.push_line(line),
                RunnerMsg::Step(step) => {
                    // Parada do operador (Stopping) nunca vira Falha: o
                    // cancelamento é intenção, não erro. (O flag `cancel`
                    // já aborta os workers; isto é o cinto.)
                    if !step.ok && !matches!(self.status, RunnerStatus::Stopping { .. }) {
                        self.status = RunnerStatus::Failed {
                            detail: format!("{}: {}", step.step, step.detail),
                        };
                    }
                    self.steps.push(step);
                }
                RunnerMsg::Spawned { child, pid } => {
                    self.child = Some(child);
                    self.status = RunnerStatus::Running {
                        pid,
                        since: std::time::Instant::now(),
                        healthy: None,
                    };
                }
                RunnerMsg::Finished => {
                    if matches!(self.status, RunnerStatus::Starting) {
                        self.status = RunnerStatus::Stopped;
                    }
                }
                RunnerMsg::Health(ok, ms) => {
                    if let RunnerStatus::Running { healthy, .. } = &mut self.status {
                        *healthy = Some((ok, ms));
                    }
                }
                RunnerMsg::ShaDone { path, hex } => {
                    self.sha_busy = false;
                    if self.sha_for == path {
                        self.sha_hex = Some(hex);
                    }
                }
                RunnerMsg::BuildDone { exe, line } => {
                    self.build_busy = false;
                    if self.build_for == exe {
                        self.build_line = line;
                    }
                }
            }
        }
        // Órfão de etapa falhada: sem propósito, SIGKILL direto + nota.
        if matches!(self.status, RunnerStatus::Failed { .. }) {
            if let Some(child) = self.child.as_mut() {
                if matches!(child.try_wait(), Ok(None)) {
                    let _ = child.kill();
                    self.push_line(String::from(
                        "runner: processo órfão encerrado (SIGKILL após falha)",
                    ));
                }
            }
        }
        // Observa a saída do filho (reaping real).
        let exited = self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok().flatten());
        if let Some(status) = exited {
            let code = status
                .code()
                .map_or(String::from("sinal"), |code| format!("exit {code}"));
            self.push_line(format!("runner: processo encerrado ({code})"));
            self.child = None;
            self.status = RunnerStatus::Stopped;
            if self.autostart {
                self.autostart = false;
                self.start();
            }
        }
        // Escalada do SIGTERM ignorado.
        if let RunnerStatus::Stopping { since, .. } = &self.status {
            if since.elapsed().as_secs() >= STOP_GRACE_SECS {
                if let Some(child) = self.child.as_mut() {
                    let _ = child.kill();
                    self.push_line(String::from("runner: SIGTERM ignorado, SIGKILL enviado"));
                }
            }
        }
        // VRAM via sysfs (barato; 1 Hz chega).
        let vram_due = self
            .last_vram_poll
            .is_none_or(|at| at.elapsed().as_secs() >= 1);
        if vram_due {
            self.last_vram_poll = Some(std::time::Instant::now());
            let (label, vram) = probe_gpu();
            self.gpu_label = label;
            self.vram = vram;
        }
        // `/health` vivo (one-shot a cada 2 s, só com filho).
        let health_due = self
            .last_health_poll
            .is_none_or(|at| at.elapsed().as_secs() >= 2);
        if health_due && self.child.is_some() {
            self.last_health_poll = Some(std::time::Instant::now());
            let base = self.params.base_url();
            if let Some(tx) = self.sender.clone() {
                std::thread::spawn(move || {
                    let (ok, ms) = match health_once(&base) {
                        Ok(ms) => (true, ms),
                        Err(_) => (false, 0),
                    };
                    let _ = tx.send(RunnerMsg::Health(ok, ms));
                });
            }
        }
    }
}

impl Default for RunnerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Linha de build do executável (`--version` real, 3 s de orçamento):
/// prefere a linha `Device` (GPU detectada), senão `version:`, senão a
/// primeira linha não vazia (90 chars). Nunca inventa.
fn probe_build(exe: &str) -> String {
    let mut child = match Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return String::from("—"),
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                return String::from("—");
            }
        }
    }
    let mut output = String::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = BufReader::new(stdout).read_to_string(&mut output);
    }
    if let Some(stderr) = child.stderr.take() {
        let _ = BufReader::new(stderr).read_to_string(&mut output);
    }
    let lines: Vec<&str> = output.lines().collect();
    if let Some(device) = lines.iter().find(|line| line.contains("Device ")) {
        return truncate(device.trim(), 90);
    }
    if let Some(version) = lines.iter().find(|line| line.contains("version:")) {
        return truncate(version.trim(), 90);
    }
    lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map_or_else(|| String::from("—"), |line| truncate(line, 90))
}

fn truncate(raw: &str, max: usize) -> String {
    if raw.len() <= max {
        return String::from(raw);
    }
    let end = raw
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|i| *i <= max)
        .last()
        .unwrap_or(0);
    format!("{}…", raw[..end].trim_end())
}

/// Drena um pipe do filho para o console (thread própria; stderr prefixado).
fn spawn_reader(
    stream: Option<impl Read + Send + 'static>,
    prefix: &'static str,
    tx: &mpsc::Sender<RunnerMsg>,
) {
    let Some(stream) = stream else { return };
    let tx = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let text = if prefix.is_empty() {
                line
            } else {
                format!("{prefix}{line}")
            };
            if tx.send(RunnerMsg::Line(text)).is_err() {
                break;
            }
        }
    });
}

/// As 5 etapas canônicas (PRD 3.4) sobre o subprocesso LOCAL: ① validar,
/// ② spawn, ③ `/health`, ④ `/v1/models`, ⑤ prova de geração. Roda em worker;
/// `cancel` aborta entre tentativas (Parar durante a subida).
fn run_steps(
    params: RunnerParams,
    spawner: impl FnOnce(&RunnerParams) -> std::io::Result<Child>,
    tx: mpsc::Sender<RunnerMsg>,
    cancel: Arc<AtomicBool>,
) {
    use std::time::Instant;
    let timed = |step: StepResult, started: Instant| {
        let mut step = step;
        step.duration_ms = started.elapsed().as_millis() as u64;
        step
    };
    let fail = |step: &'static str, detail: String| StepResult {
        step,
        ok: false,
        detail,
        duration_ms: 0,
    };
    let done = |step: &'static str, detail: String| StepResult {
        step,
        ok: true,
        detail,
        duration_ms: 0,
    };
    let cancelled = || cancel.load(Ordering::Relaxed);

    // ① Validar binário + modelo + porta (+ build real).
    let t0 = Instant::now();
    let build = probe_build(&params.exe);
    let model_bytes = std::fs::metadata(params.model.trim())
        .map(|meta| meta.len())
        .unwrap_or(0);
    if cancelled() {
        let _ = tx.send(RunnerMsg::Finished);
        return;
    }
    let _ = tx.send(RunnerMsg::Step(timed(
        done(
            "① validar binário + modelo + porta",
            format!(
                "{} · modelo {:.2} GB · porta {} livre · build: {}",
                params.exe,
                model_bytes as f64 / 1_000_000_000.0,
                params.port,
                build
            ),
        ),
        t0,
    )));

    // ② Spawn + leitores de stdout/stderr.
    let t1 = Instant::now();
    let mut child = match spawner(&params) {
        Ok(child) => child,
        Err(err) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail("② spawn do processo", err.to_string()),
                t1,
            )));
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
    };
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let _ = tx.send(RunnerMsg::Spawned { child, pid });
    spawn_reader(stdout, "", &tx);
    spawn_reader(stderr, "[stderr] ", &tx);
    let _ = tx.send(RunnerMsg::Step(timed(
        done("② spawn do processo", format!("PID {pid} ativo")),
        t1,
    )));

    let base = params.base_url();

    // ③ Aguardar `/health` (200).
    let t2 = Instant::now();
    let health_budget = params.health_timeout_secs.max(1);
    let deadline = t2 + std::time::Duration::from_secs(health_budget);
    let mut healthy_ms = None;
    while Instant::now() < deadline {
        if cancelled() {
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
        match health_once(&base) {
            Ok(ms) => {
                healthy_ms = Some(ms);
                break;
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(500)),
        }
    }
    let Some(healthy_ms) = healthy_ms else {
        let _ = tx.send(RunnerMsg::Step(timed(
            fail("③ aguardar /health", format!("sem 200 em {health_budget}s")),
            t2,
        )));
        let _ = tx.send(RunnerMsg::Finished);
        return;
    };
    let _ = tx.send(RunnerMsg::Step(timed(
        done("③ aguardar /health", format!("200 OK em {healthy_ms}ms")),
        t2,
    )));

    // ④ Modelo alocado (`/v1/models` não vazio).
    let t3 = Instant::now();
    if cancelled() {
        let _ = tx.send(RunnerMsg::Finished);
        return;
    }
    let model = match list_models(&base) {
        Ok(models) if !models.is_empty() => {
            let id = models[0].id.clone();
            let _ = tx.send(RunnerMsg::Step(timed(
                done("④ modelo & KV cache (/v1/models)", format!("modelo: {id}")),
                t3,
            )));
            id
        }
        Ok(_) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail(
                    "④ modelo & KV cache (/v1/models)",
                    String::from("servidor sem modelos anunciados"),
                ),
                t3,
            )));
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
        Err(err) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail("④ modelo & KV cache (/v1/models)", err.to_string()),
                t3,
            )));
            let _ = tx.send(RunnerMsg::Finished);
            return;
        }
    };

    // ⑤ Prova de geração real.
    let t4 = Instant::now();
    if cancelled() {
        let _ = tx.send(RunnerMsg::Finished);
        return;
    }
    let proof = chat_completion(
        &base,
        &ChatRequest {
            model,
            messages: vec![Message {
                role: Role::User,
                content: String::from(PROOF_PROMPT),
                ts_unix_ms: crate::machines::now_unix_ns() / 1_000_000,
            }],
            temperature: 0.0,
            top_p: 1.0,
            max_tokens: 32,
        },
    );
    match proof {
        Ok(reply) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                done("⑤ prova de geração", reply),
                t4,
            )));
        }
        Err(err) => {
            let _ = tx.send(RunnerMsg::Step(timed(
                fail("⑤ prova de geração", err.to_string()),
                t4,
            )));
        }
    }
    let _ = tx.send(RunnerMsg::Finished);
}

#[cfg(test)]
mod tests {
    use super::{fmt_gb, fmt_uptime, probe_build, truncate, Preset, RunnerParams, RunnerState};

    #[test]
    fn params_build_real_llama_argv() {
        let params = RunnerParams {
            exe: String::from("/usr/bin/llama-server"),
            model: String::from("/m/q.gguf"),
            port: 8081,
            ctx: 1024,
            gpu_layers: 3,
            batch: 64,
            threads: 4,
            bind: String::from("127.0.0.1"),
            health_timeout_secs: 180,
        };
        assert_eq!(
            params.args(),
            vec![
                "-m",
                "/m/q.gguf",
                "--port",
                "8081",
                "--ctx-size",
                "1024",
                "--n-gpu-layers",
                "3",
                "--batch-size",
                "64",
                "--threads",
                "4",
                "--host",
                "127.0.0.1",
            ]
        );
        assert!(params.cli_preview().starts_with("/usr/bin/llama-server -m"));
        assert_eq!(params.base_url(), "http://127.0.0.1:8081");
    }

    #[test]
    fn wildcard_bind_probes_via_loopback() {
        let mut params = RunnerParams::new();
        params.bind = String::from("0.0.0.0");
        params.port = 9099;
        assert_eq!(params.base_url(), "http://127.0.0.1:9099");
    }

    #[test]
    fn presets_apply_documented_values() {
        let mut state = RunnerState::new();
        state.apply_preset(Preset::Latencia);
        assert_eq!(state.params.ctx, 8192);
        assert_eq!(state.params.batch, 1024);
        state.apply_preset(Preset::Precisao);
        assert_eq!(state.params.ctx, 32768);
        assert_eq!(state.params.batch, 512);
    }

    #[test]
    fn validation_catches_empty_and_missing_before_any_effect() {
        let mut state = RunnerState::new();
        state.params.exe.clear();
        assert!(state.validate().is_err());
        state.params.exe = String::from("/bin/echo");
        state.params.model.clear();
        assert!(state.validate().is_err());
        state.params.model = String::from("/definitivamente/ausente.gguf");
        assert!(state.validate().unwrap_err().contains("ausente"));
    }

    #[test]
    fn occupied_port_fails_validation_honestly() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("porta efêmera liga");
        let port = listener.local_addr().expect("addr").port();
        let mut state = RunnerState::new();
        state.params.exe = String::from("/bin/echo");
        state.params.model = String::from("/bin/echo");
        state.params.bind = String::from("127.0.0.1");
        state.params.port = port;
        assert!(state.validate().unwrap_err().contains("ocupada"));
    }

    #[test]
    fn build_probe_reads_real_version_output() {
        // `/bin/true --version` existe e responde (coreutils) — a sonda lê.
        let line = probe_build("/bin/true");
        assert!(
            !line.is_empty() && line != "—",
            "true tem --version: {line}"
        );
        assert_eq!(probe_build("/definitivamente/ausente"), "—");
    }

    #[test]
    fn health_once_measures_real_round_trip() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback liga");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in listener.incoming().flatten().take(2) {
                let mut stream = stream;
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
            }
        });
        let base = format!("http://127.0.0.1:{port}");
        let ms = super::health_once(&base).expect("stub 200 responde");
        assert!(ms < 5_000, "localhost é rápido: {ms}ms");
        assert!(super::health_once("http://127.0.0.1:1").is_err());
    }

    #[test]
    fn helpers_format_without_panic_on_unicode() {
        assert_eq!(fmt_uptime(45), "45s");
        assert_eq!(fmt_uptime(192), "3m 12s");
        assert_eq!(fmt_uptime(7500), "2h 05m");
        assert_eq!(fmt_gb(5_420_000_000), "5.4");
        assert_eq!(truncate("abcdefghij", 4), "abcd…");
        assert_eq!(truncate("ações", 4), "aç…");
    }
}
