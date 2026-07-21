//! QoS Store — armazenamento em memória para métricas QoS.
//!
//! Substitui `qos_collector/qos_store.py` com DashMap em memória.

use dashmap::DashMap;

/// Store em memória para métricas QoS.
///
/// `ahash` em vez do hasher default (Fase 2 do `OPTIMIZATION_PLAN.md`).
#[derive(Default)]
pub struct QosStore {
    metrics: DashMap<String, serde_json::Value, ahash::RandomState>,
    violations: DashMap<String, serde_json::Value, ahash::RandomState>,
    discoveries: DashMap<String, serde_json::Value, ahash::RandomState>,
}

impl QosStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Armazena uma métrica QoS.
    pub fn upsert_metric(&self, key: String, value: serde_json::Value) {
        self.metrics.insert(key, value);
    }

    /// Armazena uma violação QoS.
    pub fn upsert_violation(&self, key: String, value: serde_json::Value) {
        self.violations.insert(key, value);
    }

    /// Armazena um evento de discovery.
    pub fn upsert_discovery(&self, key: String, value: serde_json::Value) {
        self.discoveries.insert(key, value);
    }

    /// Retorna todas as métricas.
    pub fn all_metrics(&self) -> Vec<serde_json::Value> {
        self.metrics.iter().map(|e| e.value().clone()).collect()
    }

    /// Retorna todas as violações.
    pub fn all_violations(&self) -> Vec<serde_json::Value> {
        self.violations.iter().map(|e| e.value().clone()).collect()
    }

    /// Retorna todos os discoveries.
    pub fn all_discoveries(&self) -> Vec<serde_json::Value> {
        self.discoveries.iter().map(|e| e.value().clone()).collect()
    }

    /// Número total de métricas.
    pub fn metrics_count(&self) -> usize {
        self.metrics.len()
    }

    /// Número total de violações.
    pub fn violations_count(&self) -> usize {
        self.violations.len()
    }

    /// Número total de discoveries.
    pub fn discoveries_count(&self) -> usize {
        self.discoveries.len()
    }

    /// Limpa todos os dados.
    pub fn clear(&self) {
        self.metrics.clear();
        self.violations.clear();
        self.discoveries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qos_store_basic() {
        let store = QosStore::new();
        assert_eq!(store.metrics_count(), 0);

        store.upsert_metric("m1".into(), serde_json::json!({"value": 1}));
        store.upsert_metric("m2".into(), serde_json::json!({"value": 2}));
        assert_eq!(store.metrics_count(), 2);

        store.upsert_violation("v1".into(), serde_json::json!({"type": "deadline"}));
        assert_eq!(store.violations_count(), 1);

        store.upsert_discovery("d1".into(), serde_json::json!({"event": "matched"}));
        assert_eq!(store.discoveries_count(), 1);

        let metrics = store.all_metrics();
        assert_eq!(metrics.len(), 2);

        store.clear();
        assert_eq!(store.metrics_count(), 0);
        assert_eq!(store.violations_count(), 0);
    }
}
