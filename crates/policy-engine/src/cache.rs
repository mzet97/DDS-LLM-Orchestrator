//! Cache local de políticas com TTL — análogo ao `_NullRedis` do Python:
//! sempre disponível e sem dependência externa. A diferença é que este cache
//! é REAL (dashmap em memória): o Rust não precisa do fallback no-op porque
//! nunca depende de um Redis externo.

use std::time::Duration;

use dashmap::DashMap;
use serde_json::Value;

use crate::now_ms;

/// TTL padrão das entradas (300 s — fiel a `ttl_seconds=300` do Python).
pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
struct CacheEntry {
    value: Value,
    expires_at_ms: u64,
}

/// Cache de políticas por `policy_id`, com expiração por TTL.
///
/// `ahash` em vez do hasher default (Fase 2 do `OPTIMIZATION_PLAN.md`).
#[derive(Debug, Default)]
pub struct PolicyCache {
    entries: DashMap<String, CacheEntry, ahash::RandomState>,
}

impl PolicyCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lê o documento cacheado; entrada expirada é removida e retorna `None`.
    pub fn get(&self, policy_id: &str) -> Option<Value> {
        self.get_at(policy_id, now_ms())
    }

    /// `get` com clock injetado (ms desde epoch) — para testes de TTL.
    #[doc(hidden)]
    pub fn get_at(&self, policy_id: &str, now: u64) -> Option<Value> {
        let entry = self.entries.get(policy_id)?;
        if entry.expires_at_ms <= now {
            drop(entry);
            self.entries.remove(policy_id);
            None
        } else {
            Some(entry.value.clone())
        }
    }

    /// Cacheia o documento com o TTL informado.
    pub fn set(&self, policy_id: &str, value: Value, ttl: Duration) {
        self.set_at(policy_id, value, ttl, now_ms());
    }

    /// `set` com clock injetado (ms desde epoch) — para testes de TTL.
    #[doc(hidden)]
    pub fn set_at(&self, policy_id: &str, value: Value, ttl: Duration, now: u64) {
        let expires_at_ms = now.saturating_add(ttl.as_millis() as u64);
        self.entries.insert(
            policy_id.to_string(),
            CacheEntry {
                value,
                expires_at_ms,
            },
        );
    }

    /// Remove a entrada; retorna se ela existia.
    pub fn delete(&self, policy_id: &str) -> bool {
        self.entries.remove(policy_id).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
