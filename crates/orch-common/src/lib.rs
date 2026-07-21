//! # orch-common
//!
//! Tipos, config, métricas e instrumentação compartilhados. Substitui
//! `src/orchestrator/common/` (Python).
//!
//! ## Ganhos de Rust já aplicáveis aqui
//! - **`tracing`** estruturado (JSON) em vez de `logging` — mesmo trace de decisão
//!   de QoS (`qos_decision`) que instrumentei no Python, agora sem custo de GIL.
//! - **Contadores atômicos / `parking_lot`** para métricas concorrentes — o bug
//!   C3 (RTTTracker/ErrorTracker sem lock, corrompendo latência) desaparece: em
//!   Rust as métricas são `AtomicU64`/`Mutex` sem GIL, corretas por construção.

/// Estados de tarefa (espelha `TaskStatus` do IDL/Python).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Done,
    Failed,
}

/// Vetor de métricas de estado do sistema (as 8 entradas do NFCM).
/// Coletado pelo control loop; alimenta o decisor de QoS e o trace de treino.
#[derive(Debug, Clone, Copy, Default)]
pub struct FuzzyMetrics {
    pub urgency: f64,
    pub deadline_pressure: f64,
    pub recent_latency: f64,
    pub agent_load: f64,
    pub error_rate: f64,
    pub historical_confidence: f64,
    pub estimated_complexity: f64,
    pub streaming_need: f64,
}

impl FuzzyMetrics {
    /// Ordem canônica das métricas (igual ao `METRICS` do qos-nfcm).
    pub fn to_array(&self) -> [f64; 8] {
        [
            self.urgency,
            self.deadline_pressure,
            self.recent_latency,
            self.agent_load,
            self.error_rate,
            self.historical_confidence,
            self.estimated_complexity,
            self.streaming_need,
        ]
    }
}

/// Instrumentação: spans de latência, contadores de RTT, rastreamento de erros.
/// Substitui `common/instrumentation.py` com contadores atômicos (sem lock).
pub mod instrumentation {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// Span de latência para decomposição T1-T6 (serialização, transporte, fila,
    /// inferência, transporte de volta, deserialização).
    #[derive(Debug)]
    pub struct LatencySpan {
        pub name: &'static str,
        start: Instant,
    }

    impl LatencySpan {
        /// Inicia um span.
        pub fn start(name: &'static str) -> Self {
            Self {
                name,
                start: Instant::now(),
            }
        }

        /// Finaliza o span e retorna a duração em nanossegundos.
        pub fn finish(self) -> u64 {
            self.start.elapsed().as_nanos() as u64
        }
    }

    /// Contador de RTT com média exponencial (EMA).
    #[derive(Debug)]
    pub struct RttTracker {
        count: AtomicU64,
        total_ns: AtomicU64,
        ema_ns: AtomicU64,
    }

    impl Default for RttTracker {
        fn default() -> Self {
            Self::new()
        }
    }

    impl RttTracker {
        pub fn new() -> Self {
            Self {
                count: AtomicU64::new(0),
                total_ns: AtomicU64::new(0),
                ema_ns: AtomicU64::new(0),
            }
        }

        /// Registra uma medição de RTT.
        pub fn record(&self, rtt_ns: u64) {
            self.count.fetch_add(1, Ordering::Relaxed);
            self.total_ns.fetch_add(rtt_ns, Ordering::Relaxed);
            // EMA: new = 0.9 * old + 0.1 * observed
            let old = self.ema_ns.load(Ordering::Relaxed);
            let new_val = if old == 0 {
                rtt_ns
            } else {
                (old as f64 * 0.9 + rtt_ns as f64 * 0.1) as u64
            };
            self.ema_ns.store(new_val, Ordering::Relaxed);
        }

        /// Retorna RTT médio em nanossegundos.
        pub fn mean_ns(&self) -> u64 {
            let count = self.count.load(Ordering::Relaxed);
            self.total_ns
                .load(Ordering::Relaxed)
                .checked_div(count)
                .unwrap_or(0)
        }

        /// Retorna EMA do RTT em nanossegundos.
        pub fn ema_ns(&self) -> u64 {
            self.ema_ns.load(Ordering::Relaxed)
        }

        /// Retorna número de medições.
        pub fn count(&self) -> u64 {
            self.count.load(Ordering::Relaxed)
        }

        /// Reseta o tracker.
        pub fn reset(&self) {
            self.count.store(0, Ordering::Relaxed);
            self.total_ns.store(0, Ordering::Relaxed);
            self.ema_ns.store(0, Ordering::Relaxed);
        }
    }

    /// Contador de erros por categoria.
    #[derive(Debug)]
    pub struct ErrorCounter {
        total: AtomicU64,
        by_category: [AtomicU64; 8], // até 8 categorias
    }

    impl Default for ErrorCounter {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ErrorCounter {
        pub fn new() -> Self {
            Self {
                total: AtomicU64::new(0),
                by_category: std::array::from_fn(|_| AtomicU64::new(0)),
            }
        }

        /// Registra um erro em uma categoria (0-7).
        pub fn record(&self, category: usize) {
            self.total.fetch_add(1, Ordering::Relaxed);
            if category < self.by_category.len() {
                self.by_category[category].fetch_add(1, Ordering::Relaxed);
            }
        }

        /// Retorna total de erros.
        pub fn total(&self) -> u64 {
            self.total.load(Ordering::Relaxed)
        }

        /// Retorna erros por categoria.
        pub fn by_category(&self, category: usize) -> u64 {
            if category < self.by_category.len() {
                self.by_category[category].load(Ordering::Relaxed)
            } else {
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_variants() {
        assert_ne!(TaskStatus::Pending, TaskStatus::Running);
        assert_ne!(TaskStatus::Done, TaskStatus::Failed);
    }

    #[test]
    fn fuzzy_metrics_to_array() {
        let m = FuzzyMetrics {
            urgency: 0.1,
            deadline_pressure: 0.2,
            recent_latency: 0.3,
            agent_load: 0.4,
            error_rate: 0.5,
            historical_confidence: 0.6,
            estimated_complexity: 0.7,
            streaming_need: 0.8,
        };
        let arr = m.to_array();
        assert_eq!(arr, [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
    }

    #[test]
    fn rtt_tracker_ema() {
        let rt = instrumentation::RttTracker::new();
        rt.record(1000);
        assert_eq!(rt.ema_ns(), 1000);
        rt.record(2000);
        // EMA: 0.9 * 1000 + 0.1 * 2000 = 1100
        assert_eq!(rt.ema_ns(), 1100);
        assert_eq!(rt.count(), 2);
    }

    #[test]
    fn error_counter_basic() {
        let ec = instrumentation::ErrorCounter::new();
        ec.record(0);
        ec.record(0);
        ec.record(3);
        assert_eq!(ec.total(), 3);
        assert_eq!(ec.by_category(0), 2);
        assert_eq!(ec.by_category(3), 1);
        assert_eq!(ec.by_category(7), 0);
    }

    #[test]
    fn latency_span_measures_time() {
        let span = instrumentation::LatencySpan::start("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ns = span.finish();
        assert!(ns >= 10_000_000); // at least 10ms
    }
}
