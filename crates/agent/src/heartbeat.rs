//! Heartbeat dedicado (REQ-205, T-206).
//!
//! Publica AgentState a cada 5s com QoS ManualByTopic(10s).
//! Não congela durante inferência longa.

use dds_contract::generated::dds_llm_orchestrator::AgentState;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Estado compartilhado do agente para heartbeat.
#[derive(Debug)]
pub struct AgentStatus {
    pub agent_id: String,
    pub hostname: String,
    pub model: String,
    pub specialization: String,
    pub slots_total: u32,
    pub slots_busy: AtomicU32,
    pub completed_total: AtomicU64,
    pub failed_total: AtomicU64,
    pub ema_latency_ms: AtomicU32, // float32 em ms, armazenado como u32 * 1000
    pub vram_total_mb: u32,
    pub vram_used_mb: AtomicU32,
    started_at: std::time::Instant,
}

impl AgentStatus {
    pub fn new(
        agent_id: String,
        hostname: String,
        model: String,
        specialization: String,
        slots: u32,
    ) -> Self {
        Self {
            agent_id,
            hostname,
            model,
            specialization,
            slots_total: slots,
            slots_busy: AtomicU32::new(0),
            completed_total: AtomicU64::new(0),
            failed_total: AtomicU64::new(0),
            ema_latency_ms: AtomicU32::new(0),
            vram_total_mb: 0,
            vram_used_mb: AtomicU32::new(0),
            started_at: std::time::Instant::now(),
        }
    }

    /// Detecta VRAM disponível via sysfs (NVIDIA/AMD) ou fallback.
    pub fn detect_vram(&mut self) {
        // Tenta NVIDIA via nvidia-smi
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=memory.total,memory.used",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 2 {
                        if let (Ok(total), Ok(used)) =
                            (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                        {
                            self.vram_total_mb = total;
                            self.vram_used_mb.store(used, Ordering::Relaxed);
                            return;
                        }
                    }
                }
            }
        }

        // Tenta AMD via rocm-smi
        if let Ok(output) = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--csv"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse rocm-smi CSV output
                for line in stdout.lines().skip(1) {
                    // skip header
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        if let (Ok(total), Ok(used)) = (
                            parts[1].parse::<f64>().map(|v| (v / 1048576.0) as u32),
                            parts[2].parse::<f64>().map(|v| (v / 1048576.0) as u32),
                        ) {
                            self.vram_total_mb = total;
                            self.vram_used_mb.store(used, Ordering::Relaxed);
                            return;
                        }
                    }
                }
            }
        }

        // Fallback: sem detecção de VRAM
        tracing::debug!("VRAM não detectado (nem NVIDIA nem AMD)");
    }

    /// Atualiza VRAM used (chamado periodicamente ou antes de inferência).
    pub fn update_vram_usage(&self) {
        if self.vram_total_mb == 0 {
            return;
        }
        // Tenta NVIDIA
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    if let Ok(used) = line.trim().parse::<u32>() {
                        self.vram_used_mb.store(used, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// Cria AgentState para publicação DDS.
    pub fn to_dds(&self) -> AgentState {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        AgentState {
            agent_id: self.agent_id.clone(),
            hostname: self.hostname.clone(),
            model: self.model.clone(),
            specialization: self.specialization.clone(),
            slots_total: self.slots_total,
            slots_busy: self.slots_busy.load(Ordering::Relaxed),
            vram_total_mb: self.vram_total_mb,
            vram_used_mb: self.vram_used_mb.load(Ordering::Relaxed),
            ema_latency_ms: self.ema_latency_ms.load(Ordering::Relaxed) as f32 / 1000.0,
            completed_total: self.completed_total.load(Ordering::Relaxed) as u32,
            failed_total: self.failed_total.load(Ordering::Relaxed) as u32,
            health: 2, // HEALTHY
            last_update_ns: now_ns,
            uptime_seconds: self.started_at.elapsed().as_secs(),
        }
    }

    /// Registra conclusão de task.
    pub fn record_completion(&self, latency_ms: u64) {
        self.completed_total.fetch_add(1, Ordering::Relaxed);
        self.slots_busy.fetch_sub(1, Ordering::Relaxed);
        // EMA: new = 0.9 * old + 0.1 * observed
        let old = self.ema_latency_ms.load(Ordering::Relaxed);
        let new_val = (old as f64 * 0.9 + latency_ms as f64 * 0.1 * 1000.0) as u32;
        self.ema_latency_ms.store(new_val, Ordering::Relaxed);
    }

    /// Registra falha de task.
    pub fn record_failure(&self) {
        self.failed_total.fetch_add(1, Ordering::Relaxed);
        self.slots_busy.fetch_sub(1, Ordering::Relaxed);
    }

    /// Reserva slot para task.
    pub fn acquire_slot(&self) -> bool {
        let current = self.slots_busy.load(Ordering::Relaxed);
        if current < self.slots_total {
            self.slots_busy.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}
