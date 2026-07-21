//! Trackers — contadores de tokens, RTT e custo.
//!
//! Substitui `metrics/{token_counter,rtt_tracker,cost_tracker}.py` com atômicos.

use std::sync::atomic::{AtomicU64, Ordering};

/// Contador de tokens (prompt + completion).
#[derive(Debug)]
pub struct TokenCounter {
    total_prompt: AtomicU64,
    total_completion: AtomicU64,
    count: AtomicU64,
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter {
    pub fn new() -> Self {
        Self {
            total_prompt: AtomicU64::new(0),
            total_completion: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Registra tokens.
    pub fn record(&self, prompt: u64, completion: u64) {
        self.total_prompt.fetch_add(prompt, Ordering::Relaxed);
        self.total_completion
            .fetch_add(completion, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_prompt(&self) -> u64 {
        self.total_prompt.load(Ordering::Relaxed)
    }

    pub fn total_completion(&self) -> u64 {
        self.total_completion.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.total_prompt() + self.total_completion()
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.total_prompt.store(0, Ordering::Relaxed);
        self.total_completion.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

/// Tracker de RTT com EMA.
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

    /// Registra RTT.
    pub fn record(&self, rtt_ns: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(rtt_ns, Ordering::Relaxed);
        let old = self.ema_ns.load(Ordering::Relaxed);
        let new_val = if old == 0 {
            rtt_ns
        } else {
            (old as f64 * 0.9 + rtt_ns as f64 * 0.1) as u64
        };
        self.ema_ns.store(new_val, Ordering::Relaxed);
    }

    pub fn mean_ns(&self) -> u64 {
        let c = self.count.load(Ordering::Relaxed);
        self.total_ns
            .load(Ordering::Relaxed)
            .checked_div(c)
            .unwrap_or(0)
    }

    pub fn ema_ns(&self) -> u64 {
        self.ema_ns.load(Ordering::Relaxed)
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.total_ns.store(0, Ordering::Relaxed);
        self.ema_ns.store(0, Ordering::Relaxed);
    }
}

/// Tracker de erros por categoria.
#[derive(Debug)]
pub struct ErrorTracker {
    total: AtomicU64,
    by_category: [AtomicU64; 8],
}

impl Default for ErrorTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorTracker {
    pub fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            by_category: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    /// Registra erro em categoria (0-7).
    pub fn record(&self, category: usize) {
        self.total.fetch_add(1, Ordering::Relaxed);
        if category < self.by_category.len() {
            self.by_category[category].fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn by_category(&self, category: usize) -> u64 {
        if category < self.by_category.len() {
            self.by_category[category].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    pub fn reset(&self) {
        self.total.store(0, Ordering::Relaxed);
        for cat in &self.by_category {
            cat.store(0, Ordering::Relaxed);
        }
    }
}

/// Tracker de custo (USD).
#[derive(Debug)]
pub struct CostTracker {
    total_micro_usd: AtomicU64, // micro-dollars para precisão
    count: AtomicU64,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            total_micro_usd: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Registra custo em USD.
    pub fn record(&self, cost_usd: f64) {
        let micro = (cost_usd * 1_000_000.0) as u64;
        self.total_micro_usd.fetch_add(micro, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Retorna custo total em USD.
    pub fn total_usd(&self) -> f64 {
        self.total_micro_usd.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Retorna custo médio em USD.
    pub fn mean_usd(&self) -> f64 {
        let c = self.count.load(Ordering::Relaxed);
        if c == 0 {
            0.0
        } else {
            self.total_usd() / c as f64
        }
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.total_micro_usd.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_counter_basic() {
        let tc = TokenCounter::new();
        assert_eq!(tc.total(), 0);
        tc.record(100, 50);
        assert_eq!(tc.total_prompt(), 100);
        assert_eq!(tc.total_completion(), 50);
        assert_eq!(tc.total(), 150);
        assert_eq!(tc.count(), 1);
        tc.record(200, 100);
        assert_eq!(tc.total(), 450);
        assert_eq!(tc.count(), 2);
        tc.reset();
        assert_eq!(tc.total(), 0);
    }

    #[test]
    fn rtt_tracker_ema() {
        let rt = RttTracker::new();
        rt.record(1_000_000); // 1ms
        assert_eq!(rt.ema_ns(), 1_000_000);
        rt.record(2_000_000); // 2ms
                              // EMA: 0.9 * 1_000_000 + 0.1 * 2_000_000 = 1_100_000
        assert_eq!(rt.ema_ns(), 1_100_000);
        assert_eq!(rt.mean_ns(), 1_500_000);
        assert_eq!(rt.count(), 2);
    }

    #[test]
    fn error_tracker_categories() {
        let et = ErrorTracker::new();
        et.record(0);
        et.record(0);
        et.record(3);
        assert_eq!(et.total(), 3);
        assert_eq!(et.by_category(0), 2);
        assert_eq!(et.by_category(3), 1);
        assert_eq!(et.by_category(7), 0);
    }

    #[test]
    fn cost_tracker_precision() {
        let ct = CostTracker::new();
        ct.record(0.001);
        ct.record(0.002);
        assert!((ct.total_usd() - 0.003).abs() < 1e-9);
        assert!((ct.mean_usd() - 0.0015).abs() < 1e-9);
        assert_eq!(ct.count(), 2);
    }
}
