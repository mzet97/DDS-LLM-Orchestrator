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
///
/// Cada ingestão é **dual-write**: atualiza o store em memória (consulta via
/// `stats()`) e emite um [`ObservabilityEvent`] no sink (persistência JSONL
/// sem janela de perda — REQ/T-820-04). Falha de sink é logada, nunca
/// propagada: o coletor não deve morrer por erro transitório de FS.
pub struct QosCollector {
    store: Arc<QosStore>,
    sink: Arc<dyn crate::sink::EventSink>,
}

impl QosCollector {
    pub fn new(store: Arc<QosStore>, sink: Arc<dyn crate::sink::EventSink>) -> Self {
        Self { store, sink }
    }

    /// Emite um evento de observabilidade com o payload serializado no
    /// `metadata` (campos do metric/violation/discovery no topo, para query
    /// direta no JSONL). Erros de sink são logados, não propagados.
    fn emit_to_sink(
        &self,
        event_type: crate::events::EventType,
        message: &str,
        payload: serde_json::Value,
    ) {
        let mut event = crate::events::ObservabilityEvent::new(event_type);
        event.message = message.to_string();
        if let serde_json::Value::Object(map) = payload {
            event.metadata = map;
        }
        if let Err(e) = self.sink.emit(&event) {
            tracing::warn!(error = %e, event_type = %event_type, "falha ao emitir evento QoS no sink");
        }
    }

    /// Ingestão de QoS metrics.
    pub fn ingest_metric(&self, metric: &QoSMetric) {
        let payload = serde_json::json!({
            "metric_id": metric.metric_id,
            "metric_name": metric.metric_name,
            "component": metric.component,
            "value": metric.value,
            "timestamp_ns": metric.timestamp_ns,
        });
        self.store
            .upsert_metric(metric.metric_id.clone(), payload.clone());
        self.emit_to_sink(
            crate::events::EventType::QosMetric,
            &format!("QoS.Metric {} = {}", metric.metric_name, metric.value),
            payload,
        );
    }

    /// Ingestão de QoS violations.
    pub fn ingest_violation(&self, violation: &QoSViolation) {
        let payload = serde_json::json!({
            "violation_id": violation.violation_id,
            "violation_type": violation.violation_type,
            "topic_name": violation.topic_name,
            "severity": violation.severity,
            "timestamp_ns": violation.timestamp_ns,
        });
        self.store
            .upsert_violation(violation.violation_id.clone(), payload.clone());
        self.emit_to_sink(
            crate::events::EventType::QosViolation,
            &format!(
                "QoS.Violation {} em {}",
                violation.violation_type, violation.topic_name
            ),
            payload,
        );
    }

    /// Ingestão de discovery events.
    pub fn ingest_discovery(&self, event: &DiscoveryEvent) {
        let payload = serde_json::json!({
            "event_id": event.event_id,
            "event_type": event.event_type,
            "topic_name": event.topic_name,
            "timestamp_ns": event.timestamp_ns,
        });
        self.store
            .upsert_discovery(event.event_id.clone(), payload.clone());
        self.emit_to_sink(
            crate::events::EventType::QosDiscovery,
            &format!("QoS.Discovery {} em {}", event.event_type, event.topic_name),
            payload,
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

#[cfg(test)]
mod tests {
    use super::*;
    use dds_dataspace::api::DataSpaceApi;
    use dds_dataspace::in_memory::InMemoryDataSpace;
    use futures::StreamExt;

    struct NullSink;

    impl crate::sink::EventSink for NullSink {
        fn emit(
            &self,
            _event: &crate::events::ObservabilityEvent,
        ) -> Result<(), crate::sink::SinkError> {
            Ok(())
        }

        fn query(
            &self,
            _task_id: &str,
            _event_type: Option<crate::events::EventType>,
            _limit: usize,
        ) -> Result<Vec<crate::events::ObservabilityEvent>, crate::sink::SinkError> {
            Ok(vec![])
        }

        fn flush(&self) -> Result<(), crate::sink::SinkError> {
            Ok(())
        }
    }

    /// Sink que captura eventos emitidos (verificação do dual-write).
    struct CaptureSink(std::sync::Mutex<Vec<crate::events::ObservabilityEvent>>);

    impl CaptureSink {
        fn new() -> Self {
            Self(std::sync::Mutex::new(Vec::new()))
        }

        fn events(&self) -> Vec<crate::events::ObservabilityEvent> {
            self.0.lock().unwrap().clone()
        }
    }

    impl crate::sink::EventSink for CaptureSink {
        fn emit(
            &self,
            event: &crate::events::ObservabilityEvent,
        ) -> Result<(), crate::sink::SinkError> {
            self.0.lock().unwrap().push(event.clone());
            Ok(())
        }

        fn query(
            &self,
            _task_id: &str,
            _event_type: Option<crate::events::EventType>,
            _limit: usize,
        ) -> Result<Vec<crate::events::ObservabilityEvent>, crate::sink::SinkError> {
            Ok(self.events())
        }

        fn flush(&self) -> Result<(), crate::sink::SinkError> {
            Ok(())
        }
    }

    /// REQ/T-820-04: cada ingestão emite um evento no sink com o payload
    /// serializado no metadata (sem janela de perda no shutdown).
    #[test]
    fn ingest_emits_sink_event_per_ingest() {
        let sink = Arc::new(CaptureSink::new());
        let collector = QosCollector::new(Arc::new(QosStore::new()), Arc::clone(&sink) as _);

        collector.ingest_metric(&QoSMetric {
            metric_id: "m1".into(),
            metric_name: "cpu".into(),
            component: "agent-1".into(),
            value: 50,
            timestamp_ns: 10,
            ..Default::default()
        });
        collector.ingest_violation(&QoSViolation {
            violation_id: "v1".into(),
            violation_type: "deadline".into(),
            topic_name: "Tasks".into(),
            severity: "high".into(),
            timestamp_ns: 11,
            ..Default::default()
        });
        collector.ingest_discovery(&DiscoveryEvent {
            event_id: "d1".into(),
            event_type: "participant_joined".into(),
            topic_name: "Tasks".into(),
            local_entity: "a".into(),
            remote_entity: "b".into(),
            count_change: 1,
            timestamp_ns: 12,
        });

        let events = sink.events();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, crate::events::EventType::QosMetric);
        assert_eq!(events[0].metadata["metric_id"], "m1");
        assert_eq!(events[0].metadata["value"], 50);
        assert_eq!(events[1].event_type, crate::events::EventType::QosViolation);
        assert_eq!(events[1].metadata["violation_type"], "deadline");
        assert_eq!(events[2].event_type, crate::events::EventType::QosDiscovery);
        assert_eq!(events[2].metadata["topic_name"], "Tasks");

        // O store também foi atualizado (dual-write).
        assert_eq!(collector.stats().total_metrics, 1);
        assert_eq!(collector.stats().total_violations, 1);
        assert_eq!(collector.stats().total_discoveries, 1);
    }

    #[tokio::test]
    async fn discovery_ingestion_end_to_end_via_dataspace() {
        // Given: coletor + dataspace em memória (sem produtor nativo Rust —
        // o evento entra por escrita direta, como faz o qos_monitor Python)
        let collector = Arc::new(QosCollector::new(
            Arc::new(QosStore::new()),
            Arc::new(NullSink),
        ));
        let ds = InMemoryDataSpace::new();
        let mut stream = Box::pin(ds.subscribe_discovery_events());

        // When: evento publicado em QoS.Discovery
        ds.write_discovery_event(DiscoveryEvent {
            event_id: "d1".into(),
            event_type: "participant_joined".into(),
            topic_name: "Tasks".into(),
            local_entity: "a".into(),
            remote_entity: "b".into(),
            count_change: 1,
            timestamp_ns: 7,
        })
        .await
        .unwrap();

        // Then: stream entrega e a ingestão agrega
        let got = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("evento chega")
            .expect("stream aberta");
        collector.ingest_discovery(&got);
        assert_eq!(collector.stats().total_discoveries, 1);
    }
}
