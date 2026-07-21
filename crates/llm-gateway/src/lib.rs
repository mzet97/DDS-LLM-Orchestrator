//! # LLM Gateway — Roteamento a provedores (Fase 3)
//!
//! Substitui `src/orchestrator/llm_gateway/` (~1,0k LOC Python):
//! - Multi-worker real (Semaphore N, sem GIL)
//! - Roteamento de provedor: local (llama-server C++) vs cloud
//! - Cache + rate-limit + 429
//! - Failover cascading com circuit breaker (T-424)

pub mod failover;

// NB: `FailoverTarget` NÃO é reexportado daqui — é definido localmente
// abaixo (composto com `LlmProvider`/`priority`, que pertencem a este
// módulo, não ao `failover` genérico).
pub use failover::{
    CircuitBreaker, CircuitBreakerConfig, FailoverConfig, FailoverEvent, FailoverManager,
    FailoverResult, FailoverStrategy, FailoverTargetConfig, HealthStatus,
};

use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Provedor de inferência LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Local, // llama-server C++
    Cloud, // OpenRouter, etc.
}

/// Constraint de provedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConstraint {
    Any,
    LocalOnly,
    CloudOnly,
}

impl ProviderConstraint {
    pub fn matches(&self, provider: Provider) -> bool {
        match self {
            Self::Any => true,
            Self::LocalOnly => provider == Provider::Local,
            Self::CloudOnly => provider == Provider::Cloud,
        }
    }
}

/// Erro do gateway.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("rate limited: {0}")]
    RateLimited(String),
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("inference failed: {0}")]
    InferenceFailed(String),
    #[error("timeout: {0}")]
    Timeout(String),
}

/// Métricas do gateway.
#[derive(Debug)]
pub struct GatewayMetrics {
    pub total_requests: AtomicU64,
    pub cache_hits: AtomicU64,
    pub rate_limited: AtomicU64,
    pub errors: AtomicU64,
    pub failover_successes: AtomicU64,
    pub failover_failures: AtomicU64,
}

impl Default for GatewayMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            failover_successes: AtomicU64::new(0),
            failover_failures: AtomicU64::new(0),
        }
    }
}

/// Rate limiter baseado em token bucket.
pub struct RateLimiter {
    tokens: std::sync::atomic::AtomicU32,
    max_tokens: u32,
    refill_rate: f64, // tokens per second
    last_refill: std::sync::Mutex<std::time::Instant>,
}

impl RateLimiter {
    pub fn new(max_tokens: u32, refill_rate: f64) -> Self {
        Self {
            tokens: std::sync::atomic::AtomicU32::new(max_tokens),
            max_tokens,
            refill_rate,
            last_refill: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.refill();
        let current = self.tokens.load(Ordering::Relaxed);
        if current > 0 {
            self.tokens.fetch_sub(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    fn refill(&self) {
        let now = std::time::Instant::now();
        let mut last = match self.last_refill.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(), // recover from poisoned mutex
        };
        let elapsed = now.duration_since(*last).as_secs_f64();
        let new_tokens = (elapsed * self.refill_rate) as u32;
        if new_tokens > 0 {
            let current = self.tokens.load(Ordering::Relaxed);
            let new_val = std::cmp::min(current + new_tokens, self.max_tokens);
            self.tokens.store(new_val, Ordering::Relaxed);
            *last = now;
        }
    }
}

/// Cache de resultados LLM.
///
/// `ahash` em vez do hasher default (Fase 2 do `OPTIMIZATION_PLAN.md`).
pub struct LlmCache {
    cache: dashmap::DashMap<String, LLMInferenceResult, ahash::RandomState>,
    max_size: usize,
}

impl LlmCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: dashmap::DashMap::with_hasher(ahash::RandomState::default()),
            max_size,
        }
    }

    pub fn get(&self, key: &str) -> Option<LLMInferenceResult> {
        self.cache.get(key).map(|v| v.clone())
    }

    pub fn insert(&self, key: String, result: LLMInferenceResult) {
        if self.cache.len() >= self.max_size {
            // Evict oldest (simplistic — could use LRU).
            // NB: o iterador precisa morrer ANTES do remove() — segurar o
            // `iter()` do DashMap durante `remove()` causa deadlock no shard.
            let first_key = self.cache.iter().next().map(|e| e.key().clone());
            if let Some(k) = first_key {
                self.cache.remove(&k);
            }
        }
        self.cache.insert(key, result);
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// Gateway LLM multi-worker.
pub struct LlmGateway {
    worker_semaphore: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
    cache: Arc<LlmCache>,
    metrics: Arc<GatewayMetrics>,
}

impl LlmGateway {
    pub fn new(num_workers: usize, rate_limit: u32, cache_size: usize) -> Self {
        Self {
            worker_semaphore: Arc::new(Semaphore::new(num_workers)),
            rate_limiter: Arc::new(RateLimiter::new(rate_limit, rate_limit as f64)),
            cache: Arc::new(LlmCache::new(cache_size)),
            metrics: Arc::new(GatewayMetrics::new()),
        }
    }

    /// Processa uma requisição LLM.
    pub async fn process(
        &self,
        request: LLMInferenceRequest,
    ) -> Result<LLMInferenceResult, GatewayError> {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

        // Rate limit check
        if !self.rate_limiter.try_acquire() {
            self.metrics.rate_limited.fetch_add(1, Ordering::Relaxed);
            return Err(GatewayError::RateLimited(
                "gateway saturado, tente novamente".into(),
            ));
        }

        // Cache check
        let cache_key = format!("{}:{}", request.agent_id, request.model_name);
        if let Some(cached) = self.cache.get(&cache_key) {
            self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached);
        }

        // Acquire worker semaphore
        let _permit = self
            .worker_semaphore
            .acquire()
            .await
            .map_err(|e| GatewayError::ProviderUnavailable(e.to_string()))?;

        // Roteamento de provedor: use process_routed() com GatewayProviders
        // para roteamento real (local vs cloud). Este path básico retorna
        // ProviderUnavailable para forçar o uso de process_routed().
        self.metrics.errors.fetch_add(1, Ordering::Relaxed);
        Err(GatewayError::ProviderUnavailable(
            "use process_routed() com GatewayProviders para roteamento real".into(),
        ))
    }

    /// Retorna métricas do gateway.
    pub fn metrics(&self) -> &GatewayMetrics {
        &self.metrics
    }
}

// ── Roteamento de provedor (T-421/422) ─────────────────────────────────────

/// Provider de inferência (local llama-server via DDS, cloud, mock de teste).
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Executa a inferência completa (modo não-stream; para stream, o provider
    /// publica chunks em `LLM.InferenceResult` diretamente).
    async fn infer(
        &self,
        request: &LLMInferenceRequest,
    ) -> Result<LLMInferenceResult, GatewayError>;
    /// "local" | "cloud" | "mock"
    fn kind(&self) -> Provider;
}

impl GatewayError {
    /// Mapeia para `LLMInferenceError` no fio (REQ-422): rate limit → 429 retriable.
    pub fn to_llm_error(&self, request_id: &str) -> LLMInferenceError {
        let (code, msg, retriable) = match self {
            GatewayError::RateLimited(m) => (429, m.clone(), true),
            GatewayError::ProviderUnavailable(m) => (503, m.clone(), true),
            GatewayError::InferenceFailed(m) => (500, m.clone(), false),
            GatewayError::Timeout(m) => (504, m.clone(), true),
        };
        LLMInferenceError {
            request_id: request_id.into(),
            error_code: code,
            error_message: msg,
            provider: "llm-gateway".into(),
            retriable,
            emitted_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        }
    }
}

impl LlmGateway {
    /// Constrói o gateway com providers (T-421).
    pub fn with_providers(
        local: Option<Arc<dyn LlmProvider>>,
        cloud: Option<Arc<dyn LlmProvider>>,
        num_workers: usize,
        rate_limit: u32,
        cache_size: usize,
    ) -> (Self, GatewayProviders) {
        (
            Self::new(num_workers, rate_limit, cache_size),
            GatewayProviders::new(local, cloud),
        )
    }

    /// Processa uma requisição com roteamento de provedor (T-420/421/422).
    ///
    /// Failover flow: se o provider primário falhar e houver FailoverTargets
    /// configurados, tenta cada target em ordem de prioridade, respeitando
    /// circuit breakers.
    pub async fn process_routed(
        &self,
        providers: &GatewayProviders,
        request: LLMInferenceRequest,
    ) -> Result<LLMInferenceResult, GatewayError> {
        self.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

        // Cache ANTES do rate limit (T-422): cache hit não consome quota.
        let cache_key = format!(
            "{}:{}:{}:{}:{}",
            request.provider_constraint,
            request.model_name,
            request.messages_json,
            request.temperature,
            request.max_tokens
        );
        if let Some(cached) = self.cache.get(&cache_key) {
            self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached);
        }

        // Rate limit → 429 retriable (T-422)
        if !self.rate_limiter.try_acquire() {
            self.metrics.rate_limited.fetch_add(1, Ordering::Relaxed);
            return Err(GatewayError::RateLimited(
                "gateway saturado, tente novamente".into(),
            ));
        }

        // Roteamento por constraint (T-421)
        let provider = providers.route(&request.provider_constraint)?;
        let provider_name = providers.provider_name_from_constraint(&request.provider_constraint);
        let _permit = self
            .worker_semaphore
            .acquire()
            .await
            .map_err(|e| GatewayError::ProviderUnavailable(e.to_string()))?;

        // Tenta o provider primário
        let primary_result = provider.infer(&request).await;
        match primary_result {
            Ok(result) => {
                if !request.stream {
                    self.cache.insert(cache_key, result.clone());
                }
                Ok(result)
            }
            Err(primary_err) => {
                self.metrics.errors.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    provider = provider_name,
                    error = %primary_err,
                    "Provider primário falhou, verificando failover"
                );

                // Tenta failover targets
                let failover_targets = providers.get_failover_targets(provider_name);
                if failover_targets.is_empty() {
                    return Err(primary_err);
                }

                let mut last_err = primary_err;
                for target in failover_targets.iter().filter(|t| t.priority > 0) {
                    if !target.circuit_breaker.is_available() {
                        tracing::debug!(
                            model = %target.model,
                            "Failover target circuit breaker aberto, pulando"
                        );
                        continue;
                    }

                    tracing::info!(
                        model = %target.model,
                        "Tentando failover para modelo alternativo"
                    );

                    // Cria request com modelo do failover target
                    let mut failover_request = request.clone();
                    failover_request.model_name = target.model.clone();
                    failover_request.request_id = format!("{}-fo", request.request_id);

                    let failover_result = target.provider.infer(&failover_request).await;
                    match failover_result {
                        Ok(result) => {
                            target.circuit_breaker.record_success();
                            self.metrics
                                .failover_successes
                                .fetch_add(1, Ordering::Relaxed);
                            tracing::info!(
                                model = %target.model,
                                "Failover bem-sucedido"
                            );
                            if !request.stream {
                                self.cache.insert(cache_key, result.clone());
                            }
                            return Ok(result);
                        }
                        Err(e) => {
                            target.circuit_breaker.record_failure();
                            self.metrics
                                .failover_failures
                                .fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(
                                model = %target.model,
                                error = %e,
                                "Failover target falhou"
                            );
                            last_err = e;
                        }
                    }
                }

                tracing::error!(
                    provider = provider_name,
                    "Todos os targets de failover esgotados"
                );
                Err(last_err)
            }
        }
    }
}

/// Providers configurados do gateway.
#[derive(Clone)]
pub struct GatewayProviders {
    pub local: Option<Arc<dyn LlmProvider>>,
    pub cloud: Option<Arc<dyn LlmProvider>>,
    /// Failover configs: provider name → list of failover targets (ordered by priority).
    failover_configs: HashMap<String, Vec<FailoverTarget>>,
}

/// A single failover target with its circuit breaker.
#[derive(Clone)]
pub struct FailoverTarget {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    pub circuit_breaker: Arc<CircuitBreaker>,
    pub priority: u32,
}

impl GatewayProviders {
    pub fn new(local: Option<Arc<dyn LlmProvider>>, cloud: Option<Arc<dyn LlmProvider>>) -> Self {
        Self {
            local,
            cloud,
            failover_configs: HashMap::new(),
        }
    }

    /// Register failover targets for a provider.
    pub fn register_failover(&mut self, provider: &str, targets: Vec<FailoverTarget>) {
        self.failover_configs.insert(provider.to_string(), targets);
    }

    /// Get failover targets for a provider (returns empty slice if none).
    pub fn get_failover_targets(&self, provider: &str) -> &[FailoverTarget] {
        self.failover_configs
            .get(provider)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check if any failover target has its circuit breaker open.
    pub fn any_failover_available(&self, provider: &str) -> bool {
        self.get_failover_targets(provider)
            .iter()
            .any(|t| t.circuit_breaker.is_available())
    }

    pub fn route(&self, constraint: &str) -> Result<Arc<dyn LlmProvider>, GatewayError> {
        match constraint {
            "LOCAL_ONLY" => self.local.clone().ok_or_else(|| {
                GatewayError::ProviderUnavailable("provider local indisponível".into())
            }),
            "CLOUD_ONLY" => self.cloud.clone().ok_or_else(|| {
                GatewayError::ProviderUnavailable("provider cloud indisponível".into())
            }),
            _ => self
                .local
                .clone()
                .or_else(|| self.cloud.clone())
                .ok_or_else(|| {
                    GatewayError::ProviderUnavailable("nenhum provider configurado".into())
                }),
        }
    }

    /// Determine provider name from constraint for failover lookup.
    pub fn provider_name_from_constraint(&self, constraint: &str) -> &str {
        match constraint {
            "LOCAL_ONLY" => "local",
            "CLOUD_ONLY" => "cloud",
            _ => "local", // default
        }
    }
}

/// Provider de teste: responde com delay e conta concorrência máxima.
/// Se `fail` for true, sempre retorna erro (para testar failover).
pub struct MockProvider {
    pub kind: Provider,
    pub delay: std::time::Duration,
    pub concurrent: Arc<std::sync::atomic::AtomicUsize>,
    pub max_concurrent: Arc<std::sync::atomic::AtomicUsize>,
    pub calls: Arc<std::sync::atomic::AtomicU64>,
    pub fail: bool,
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn infer(
        &self,
        request: &LLMInferenceRequest,
    ) -> Result<LLMInferenceResult, GatewayError> {
        let cur = self.concurrent.fetch_add(1, Ordering::Relaxed) + 1;
        self.max_concurrent.fetch_max(cur, Ordering::Relaxed);
        self.calls.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(self.delay).await;
        self.concurrent.fetch_sub(1, Ordering::Relaxed);
        if self.fail {
            return Err(GatewayError::InferenceFailed("mock failure".into()));
        }
        Ok(LLMInferenceResult {
            request_id: request.request_id.clone(),
            seq_num: 0,
            content: format!("mock-{}", request.request_id),
            is_final: true,
            finish_reason: 1,
            model_used: request.model_name.clone(),
            tokens_prompt: 10,
            tokens_completion: 4,
            emitted_at_ns: 0,
        })
    }

    fn kind(&self) -> Provider {
        self.kind
    }
}
