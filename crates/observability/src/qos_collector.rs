//! QoS Collector — agregador de métricas/violações/discovery.
//!
//! Substitui `qos_collector/main.py` com DashMap em memória.

use crate::qos_store::QosStore;
use dds_contract::generated::dds_llm_orchestrator::{DiscoveryEvent, QoSMetric, QoSViolation};
use std::sync::Arc;

/// Estatísticas do coletor.
#[derive(Debug, Clone, Default)]
pub struct CollectorStats {
    pub total_metrics: u64,
    pub total_violations: u64,
    pub total_discoveries: u64,
}

/// Agregador de QoS metrics, violations e discovery events.
pub struct QosCollector {
    store: Arc<QosStore>,
    sink: Arc<dyn crate::sink::EventSink>,
    stats: std::sync::atomic::AtomicU64,
}

impl QosCollector {
    pub fn new(store: Arc<QosStore>, sink: Arc<dyn crate::sink::EventSink>) -> Self {
        Self {
            store,
            sink,
            stats: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Ingestão de QoS metrics.
    pub fn ingest_metric(&self, metric: &QoSMetric) {
        self.store.upsert_metric(
            metric.metric_id.clone(),
            serde_json::json!({
                "metric_id": metric.metric_id,
                "metric_name": metric.metric_name,
                "component": metric.component,
                "value": metric.value,
                "timestamp_ns": metric.timestamp_ns,
            }),
        );
        self.stats
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Ingestão de QoS violations.
    pub fn ingest_violation(&self, violation: &QoSViolation) {
        self.store.upsert_violation(
            violation.violation_id.clone(),
            serde_json::json!({
                "violation_id": violation.violation_id,
                "violation_type": violation.violation_type,
                "topic_name": violation.topic_name,
                "severity": violation.severity,
                "timestamp_ns": violation.timestamp_ns,
            }),
        );
    }

    /// Ingestão de discovery events.
    pub fn ingest_discovery(&self, event: &DiscoveryEvent) {
        self.store.upsert_discovery(
            event.event_id.clone(),
            serde_json::json!({
                "event_id": event.event_id,
                "event_type": event.event_type,
                "topic_name": event.topic_name,
                "timestamp_ns": event.timestamp_ns,
            }),
        );
    }

    /// Flush explícito do sink (paridade com o flush periódico do Python).
    /// Erros de escrita são logados, não propagados — o coletor não deve
    /// morrer por falha transitória de FS.
    pub fn flush(&self) {
        if let Err(e) = self.sink.flush() {
            tracing::warn!(error = %e, "falha no flush do sink");
        }
    }

    /// Estatísticas do coletor.
    pub fn stats(&self) -> CollectorStats {
        CollectorStats {
            total_metrics: self.store.metrics_count() as u64,
            total_violations: self.store.violations_count() as u64,
            total_discoveries: self.store.discoveries_count() as u64,
        }
    }

    /// Spawn ingestion loop via DDS streams.
    ///
    /// Consome `QoS.Metric`/`QoS.Violation`/`QoS.Discovery` e faz flush do sink
    /// a cada `flush_interval` (paridade com o flush periódico do Python).
    /// Recebe `Arc<Self>` para que o chamador possa continuar lendo `stats()`.
    #[cfg(feature = "dds")]
    pub fn spawn_ingestion(
        self: Arc<Self>,
        dataspace: Arc<dds_dataspace::DataSpace>,
        flush_interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut metrics_stream = Box::pin(dataspace.stream_qos_metrics());
            let mut violations_stream = Box::pin(dataspace.stream_qos_violations());
            let mut discovery_stream = Box::pin(dataspace.stream_discovery_events());
            let mut flush_tick = tokio::time::interval(flush_interval);

            loop {
                tokio::select! {
                    Some(metric) = metrics_stream.next() => {
                        self.ingest_metric(&metric);
                    }
                    Some(violation) = violations_stream.next() => {
                        self.ingest_violation(&violation);
                    }
                    Some(event) = discovery_stream.next() => {
                        self.ingest_discovery(&event);
                    }
                    _ = flush_tick.tick() => {
                        self.flush();
                    }
                }
            }
        })
    }
}
