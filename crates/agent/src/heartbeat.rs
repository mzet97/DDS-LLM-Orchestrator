//! Heartbeat dedicado (REQ-205, T-206).
//!
//! Publica AgentState a cada 5s com QoS ManualByTopic(10s).
//! Não congela durante inferência longa.

use dds_contract::generated::dds_llm_orchestrator::AgentState;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::OwnedSemaphorePermit;

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

    /// Registra conclusão de task (métricas apenas — o slot lógico é liberado
    /// pelo [`SlotGuard`], que é o dono da contagem `slots_busy`).
    pub fn record_completion(&self, latency_ms: u64) {
        self.completed_total.fetch_add(1, Ordering::Relaxed);
        // EMA: new = 0.9 * old + 0.1 * observed
        let old = self.ema_latency_ms.load(Ordering::Relaxed);
        let new_val = (old as f64 * 0.9 + latency_ms as f64 * 0.1 * 1000.0) as u32;
        self.ema_latency_ms.store(new_val, Ordering::Relaxed);
    }

    /// Registra falha de task (métricas apenas — ver [`record_completion`]).
    pub fn record_failure(&self) {
        self.failed_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Reserva slot para task (CAS — admissão atômica, sem load+add solto).
    /// Par de [`AgentStatus::release_slot`]; em produção use [`SlotGuard`].
    pub fn acquire_slot(&self) -> bool {
        self.slots_busy
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                (cur < self.slots_total).then_some(cur + 1)
            })
            .is_ok()
    }

    /// Libera um slot lógico. Chamada exclusivamente pelo [`SlotGuard`].
    fn release_slot(&self) {
        self.slots_busy.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Erro operacional tipado de capacidade (RUST-SLOT-007): o semáforo de
/// admissão cedeu um permit, mas o contador lógico `slots_busy` estava cheio.
/// Substitui o `assert!` que panicava o agente nessa divergência.
#[derive(Debug, thiserror::Error)]
#[error("divergência de capacidade: permit do semáforo sem slot lógico livre")]
pub struct SlotUnavailable;

/// Guard RAII de capacidade (RUST-SLOT-007): amarra o permit do Semaphore de
/// admissão e o contador `slots_busy` numa única posse. Todo caminho de saída
/// (sucesso, erro `?`, panic contido, cancelamento, shutdown) libera os dois
/// via Drop. Antes deste guard, saídas antecipadas de `process_and_publish`
/// (ex.: `?` no write de RUNNING) pulavam o decremento de `slots_busy`,
/// fazendo a dupla contabilidade divergir até o `assert!` panicar.
pub struct SlotGuard {
    status: Arc<AgentStatus>,
    _permit: Option<OwnedSemaphorePermit>,
}

impl SlotGuard {
    /// Adquire o par (permit, slots_busy) como uma única unidade.
    /// `permit` é `None` no caminho não-DDS (`Agent::process_task`).
    pub fn acquire(
        permit: Option<OwnedSemaphorePermit>,
        status: Arc<AgentStatus>,
    ) -> Result<Self, SlotUnavailable> {
        if !status.acquire_slot() {
            return Err(SlotUnavailable);
        }
        Ok(Self {
            status,
            _permit: permit,
        })
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.status.release_slot();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use tokio::sync::Semaphore;

    fn make_status(slots: u32) -> Arc<AgentStatus> {
        Arc::new(AgentStatus::new(
            "agent-t".into(),
            "host-t".into(),
            "model-t".into(),
            "Text".into(),
            slots,
        ))
    }

    /// RUST-SLOT-007: sucesso e erro liberam o slot — `slots_busy` volta ao
    /// valor inicial em qualquer caminho de saída.
    #[test]
    fn guard_libera_slot_no_drop() {
        let status = make_status(2);
        {
            let _g = SlotGuard::acquire(None, Arc::clone(&status)).unwrap();
            assert_eq!(status.slots_busy.load(Ordering::Relaxed), 1);
        }
        assert_eq!(status.slots_busy.load(Ordering::Relaxed), 0);
    }

    /// RUST-SLOT-007: panic contido também libera (Drop durante unwind).
    #[test]
    fn guard_libera_slot_em_panic() {
        let status = make_status(1);
        let s2 = Arc::clone(&status);
        let r = catch_unwind(AssertUnwindSafe(move || {
            let _g = SlotGuard::acquire(None, s2).unwrap();
            panic!("falha injetada");
        }));
        assert!(r.is_err());
        assert_eq!(status.slots_busy.load(Ordering::Relaxed), 0);
    }

    /// RUST-SLOT-007: divergência permit×contador vira erro tipado, não panic
    /// (o `assert!` antigo derrubava o agente).
    #[test]
    fn divergencia_vira_erro_tipado_sem_panic() {
        let status = make_status(1);
        // Permit do semáforo em mãos, mas contador lógico já cheio:
        let sem = Arc::new(Semaphore::new(1));
        let permit = Arc::clone(&sem).try_acquire_owned().unwrap();
        status.slots_busy.store(1, Ordering::Relaxed); // divergência injetada
        let result = SlotGuard::acquire(Some(permit), Arc::clone(&status));
        assert!(matches!(result, Err(SlotUnavailable)));
        // permit foi consumido pelo guard mal-sucedido? Não: o guard não foi
        // criado, o permit cai com o Err e o semáforo se recupera.
        assert_eq!(sem.available_permits(), 1);
    }

    /// Admissão atômica (CAS): N concorrentes, exatamente `slots_total`
    /// vencem; nenhum estouro de `slots_busy` sob corrida.
    #[test]
    fn acquire_slot_cas_nao_ultrapassa_total() {
        let status = make_status(4);
        let won = Arc::new(AtomicU32::new(0));
        let mut handles = Vec::new();
        for _ in 0..16 {
            let s = Arc::clone(&status);
            let w = Arc::clone(&won);
            handles.push(std::thread::spawn(move || {
                if s.acquire_slot() {
                    w.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(won.load(Ordering::Relaxed), 4);
        assert_eq!(status.slots_busy.load(Ordering::Relaxed), 4);
        assert!(status.slots_busy.load(Ordering::Relaxed) <= status.slots_total);
    }
}
