//! Parte de `runner` (extraída para manter os arquivos revisáveis).

use super::*;

impl RunnerState {
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
