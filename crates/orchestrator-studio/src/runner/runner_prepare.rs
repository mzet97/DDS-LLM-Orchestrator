//! Parte de `runner` (extraída para manter os arquivos revisáveis).

use super::*;

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

    pub(super) fn ensure_channel(&mut self) -> Option<mpsc::Sender<RunnerMsg>> {
        if self.sender.is_none() {
            let (tx, rx) = mpsc::channel();
            self.sender = Some(tx);
            self.receiver = Some(rx);
        }
        self.sender.clone()
    }

    pub(super) fn push_line(&mut self, line: String) {
        if self.console.len() == CONSOLE_CAP {
            self.console.pop_front();
        }
        self.console.push_back(line);
    }
}
