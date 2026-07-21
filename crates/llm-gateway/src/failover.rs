//! Circuit breaker e políticas de failover (T-424).
//!
//! `lib.rs` (`GatewayProviders`/`FailoverTarget`) já orquestra a cascata de
//! failover em `process_routed()`, consultando `CircuitBreaker::is_available`
//! e alimentando `record_success`/`record_failure` por target. Este módulo
//! fornece esse breaker (3 estados: closed → open → half-open) e um
//! `FailoverManager` opcional para agrupar breakers por chave fora do
//! `GatewayProviders` (útil para health checks externos e testes).
//!
//! Substitui o equivalente informal do Python (retry manual sem estado
//! persistido entre chamadas): aqui o estado do breaker sobrevive entre
//! requisições e evita martelar um provider já degradado.

use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Estado de saúde observável de um `CircuitBreaker` (para métricas/logs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakerState {
    Closed = 0,
    Open = 1,
    HalfOpen = 2,
}

impl From<u8> for BreakerState {
    fn from(v: u8) -> Self {
        match v {
            1 => BreakerState::Open,
            2 => BreakerState::HalfOpen,
            _ => BreakerState::Closed,
        }
    }
}

/// Circuit breaker com 3 estados:
/// - **closed**: passa tudo; conta falhas consecutivas.
/// - **open**: após `failure_threshold` falhas seguidas, bloqueia por
///   `reset_after`.
/// - **half-open**: decorrido `reset_after`, libera uma sonda; sucesso volta
///   a closed, falha reabre o breaker.
#[derive(Debug)]
pub struct CircuitBreaker {
    state: AtomicU8,
    consecutive_failures: AtomicU32,
    failure_threshold: u32,
    reset_after: Duration,
    opened_at_ms: AtomicU64,
    started_at: Instant,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, reset_after: Duration) -> Self {
        Self {
            state: AtomicU8::new(BreakerState::Closed as u8),
            consecutive_failures: AtomicU32::new(0),
            failure_threshold: failure_threshold.max(1),
            reset_after,
            opened_at_ms: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    /// True se uma tentativa pode ser feita agora: closed, half-open, ou
    /// open com `reset_after` já decorrido (transiciona para half-open e
    /// libera a sonda).
    pub fn is_available(&self) -> bool {
        match BreakerState::from(self.state.load(Ordering::Acquire)) {
            BreakerState::Closed | BreakerState::HalfOpen => true,
            BreakerState::Open => {
                let opened_at = self.opened_at_ms.load(Ordering::Acquire);
                let elapsed_ms = self.now_ms().saturating_sub(opened_at);
                if elapsed_ms >= self.reset_after.as_millis() as u64 {
                    // CAS best-effort: se outra thread já transicionou, tudo bem
                    // (ambas concluem que está disponível).
                    let _ = self.state.compare_exchange(
                        BreakerState::Open as u8,
                        BreakerState::HalfOpen as u8,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Registra sucesso: fecha o breaker e zera o contador de falhas.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
        self.state
            .store(BreakerState::Closed as u8, Ordering::Release);
    }

    /// Registra falha: em half-open, reabre imediatamente (1 falha basta);
    /// em closed, reabre ao atingir `failure_threshold` consecutivas.
    pub fn record_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1;
        let was_half_open =
            BreakerState::from(self.state.load(Ordering::Acquire)) == BreakerState::HalfOpen;
        if was_half_open || failures >= self.failure_threshold {
            self.state
                .store(BreakerState::Open as u8, Ordering::Release);
            self.opened_at_ms.store(self.now_ms(), Ordering::Release);
        }
    }

    pub fn health(&self) -> HealthStatus {
        match BreakerState::from(self.state.load(Ordering::Acquire)) {
            BreakerState::Closed if self.consecutive_failures.load(Ordering::Acquire) == 0 => {
                HealthStatus::Healthy
            }
            BreakerState::Closed | BreakerState::HalfOpen => HealthStatus::Degraded,
            BreakerState::Open => HealthStatus::Unavailable,
        }
    }
}

/// Estratégia de seleção entre targets de failover disponíveis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailoverStrategy {
    /// Ordem de prioridade declarada (o que `process_routed` usa hoje).
    #[default]
    Priority,
    /// Round-robin entre os targets com breaker disponível.
    RoundRobin,
}

/// Configuração para construir um `CircuitBreaker` (thresholds independentes
/// de runtime — vem de config estática/YAML/env).
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
        }
    }
}

impl CircuitBreakerConfig {
    pub fn build(&self) -> CircuitBreaker {
        CircuitBreaker::new(self.failure_threshold, self.recovery_timeout)
    }
}

/// Configuração declarativa de UM target de failover: para qual provider
/// alternativo cair e com qual modelo, em qual prioridade, com qual breaker.
/// Consumida por `orchestrator::build_failover_chains` para montar
/// `FailoverTarget`s a partir de config estática (YAML/env) e registrá-los em
/// `GatewayProviders::register_failover`.
#[derive(Debug, Clone)]
pub struct FailoverTargetConfig {
    /// Nome do provider a resolver via `provider_factory` do chamador
    /// (tipicamente "local" | "cloud").
    pub provider: String,
    /// Modelo a usar neste target de fallback.
    pub model: String,
    pub circuit_breaker: CircuitBreakerConfig,
    /// Ordem de tentativa; `process_routed` filtra `priority > 0`.
    pub priority: u32,
}

/// Configuração declarativa de uma cadeia de failover completa: o provider
/// primário cujas falhas disparam a cascata, e a lista ordenada de targets
/// de fallback.
#[derive(Debug, Clone, Default)]
pub struct FailoverConfig {
    /// Nome do provider primário (chave usada em `GatewayProviders::route`
    /// e em `get_failover_targets`).
    pub primary_provider: String,
    pub targets: Vec<FailoverTargetConfig>,
    pub strategy: FailoverStrategy,
}

/// Resultado observável de uma tentativa de failover (para métricas/tracing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverResult {
    Success,
    Failed,
    CircuitOpen,
}

/// Evento de failover emitido para observabilidade (correlaciona com
/// `GatewayMetrics::failover_successes/failures` em `lib.rs`).
#[derive(Debug, Clone)]
pub struct FailoverEvent {
    pub provider: String,
    pub target_model: String,
    pub result: FailoverResult,
    pub at_ns: u64,
}

impl FailoverEvent {
    pub fn now(
        provider: impl Into<String>,
        target_model: impl Into<String>,
        result: FailoverResult,
    ) -> Self {
        Self {
            provider: provider.into(),
            target_model: target_model.into(),
            result,
            at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        }
    }
}

/// Gestor de circuit breakers por chave (ex.: nome do modelo/target),
/// independente do `HashMap` interno de `GatewayProviders`. Útil para health
/// checks externos (ex.: endpoint `/health/providers`) e para testes que
/// queiram inspecionar/forçar o estado de um breaker sem montar um
/// `GatewayProviders` completo.
pub struct FailoverManager {
    config: CircuitBreakerConfig,
    strategy: FailoverStrategy,
    breakers: DashMap<String, Arc<CircuitBreaker>>,
}

impl FailoverManager {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self::with_strategy(config, FailoverStrategy::Priority)
    }

    pub fn with_strategy(config: CircuitBreakerConfig, strategy: FailoverStrategy) -> Self {
        Self {
            config,
            strategy,
            breakers: DashMap::new(),
        }
    }

    /// Retorna o breaker da chave, criando-o com `self.config` na primeira vez.
    pub fn breaker(&self, key: &str) -> Arc<CircuitBreaker> {
        if let Some(existing) = self.breakers.get(key) {
            return existing.clone();
        }
        let created = Arc::new(self.config.build());
        self.breakers
            .entry(key.to_string())
            .or_insert(created)
            .clone()
    }

    pub fn is_available(&self, key: &str) -> bool {
        self.breaker(key).is_available()
    }

    pub fn record_success(&self, key: &str) {
        self.breaker(key).record_success();
    }

    pub fn record_failure(&self, key: &str) {
        self.breaker(key).record_failure();
    }

    pub fn health(&self, key: &str) -> HealthStatus {
        self.breaker(key).health()
    }

    pub fn strategy(&self) -> FailoverStrategy {
        self.strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_breaker_is_available_and_healthy() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(20));
        assert!(cb.is_available());
        assert_eq!(cb.health(), HealthStatus::Healthy);
    }

    #[test]
    fn opens_after_threshold_consecutive_failures() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(60));
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_available(), "ainda não atingiu o threshold");
        cb.record_failure();
        assert!(
            !cb.is_available(),
            "3ª falha consecutiva deve abrir o breaker"
        );
        assert_eq!(cb.health(), HealthStatus::Unavailable);
    }

    #[test]
    fn success_resets_failure_count() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(60));
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        cb.record_failure();
        cb.record_failure();
        assert!(
            cb.is_available(),
            "contador deveria ter sido zerado pelo sucesso"
        );
    }

    #[test]
    fn half_opens_after_reset_and_recloses_on_success() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(10));
        cb.record_failure();
        assert!(!cb.is_available());
        std::thread::sleep(Duration::from_millis(25));
        assert!(cb.is_available(), "deveria permitir sonda em half-open");
        cb.record_success();
        assert_eq!(cb.health(), HealthStatus::Healthy);
    }

    #[test]
    fn half_open_probe_failure_reopens_immediately() {
        let cb = CircuitBreaker::new(5, Duration::from_millis(10));
        cb.record_failure();
        // Ainda closed (threshold=5), mas isso não importa para este teste:
        // force open manualmente via várias falhas rápidas não é necessário —
        // testamos diretamente a transição half-open → open com 1 breaker
        // de threshold baixo.
        let cb2 = CircuitBreaker::new(1, Duration::from_millis(10));
        cb2.record_failure(); // abre
        std::thread::sleep(Duration::from_millis(25));
        assert!(cb2.is_available()); // half-open
        cb2.record_failure(); // 1 falha em half-open reabre
        assert!(!cb2.is_available());
        let _ = cb; // silencia unused em builds sem otimização de branch acima
    }

    #[test]
    fn failover_manager_reuses_breaker_per_key() {
        let mgr = FailoverManager::new(CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
        });
        mgr.record_failure("model-a");
        mgr.record_failure("model-a");
        assert!(!mgr.is_available("model-a"));
        // Chave diferente não é afetada.
        assert!(mgr.is_available("model-b"));
        assert_eq!(mgr.strategy(), FailoverStrategy::Priority);
    }

    #[test]
    fn failover_config_feeds_build_failover_chains_shape() {
        // Regressão: `orchestrator::build_failover_chains` desestrutura
        // `FailoverConfig{primary_provider, targets}` e cada
        // `FailoverTargetConfig{provider, model, circuit_breaker, priority}`.
        // Este teste apenas fixa a forma pública do tipo.
        let cfg = FailoverConfig {
            primary_provider: "local".into(),
            targets: vec![FailoverTargetConfig {
                provider: "cloud".into(),
                model: "backup-model".into(),
                circuit_breaker: CircuitBreakerConfig::default(),
                priority: 1,
            }],
            strategy: FailoverStrategy::Priority,
        };
        assert_eq!(cfg.primary_provider, "local");
        assert_eq!(cfg.targets.len(), 1);
        assert_eq!(cfg.targets[0].priority, 1);
    }

    #[test]
    fn failover_event_carries_result_and_timestamp() {
        let ev = FailoverEvent::now("local", "backup-model", FailoverResult::Success);
        assert_eq!(ev.provider, "local");
        assert_eq!(ev.target_model, "backup-model");
        assert_eq!(ev.result, FailoverResult::Success);
        assert!(ev.at_ns > 0);
    }
}
