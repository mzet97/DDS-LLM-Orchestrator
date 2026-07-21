//! Ingestão DDS → coletores (feature `dds`).
//!
//! Conecta os streams do [`dds_dataspace::DataSpace`] aos coletores em
//! memória, espelhando os `subscribe_qos_metrics` / `subscribe_qos_violations`
//! / `subscribe_discovery_events` / `subscribe_execution_traces` do
//! `dds_backend` Python. Nomes de tópicos e tipos são os do contrato IDL
//! (interop com a malha Python/C++).

use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::qos_collector::QosCollector;
use crate::trace_collector::TraceCollector;

/// Handles dos loops de ingestão. Abortar os handles encerra os loops
/// (o `DataSpace` em si é encerrado pelo drop do owner).
pub struct IngestionHandles {
    /// Loop de `QoS.Metric`/`QoS.Violation`/`QoS.Discovery` → [`QosCollector`].
    pub qos: JoinHandle<()>,
    /// Loop de `Execution.Trace` → [`TraceCollector`].
    pub traces: JoinHandle<()>,
}

/// Sobe os dois loops de ingestão (QoS + traces) sobre o mesmo `DataSpace`.
///
/// `flush_interval` controla o flush periódico do sink do [`QosCollector`];
/// o [`TraceCollector`] faz flush via chamada explícita do serviço (paridade
/// com o `trace_collector.py`, que grava por demanda/encerramento).
pub fn spawn_ingestion(
    dataspace: Arc<dds_dataspace::DataSpace>,
    qos_collector: Arc<QosCollector>,
    trace_collector: Arc<TraceCollector>,
    flush_interval: Duration,
) -> IngestionHandles {
    let qos = qos_collector.spawn_ingestion(Arc::clone(&dataspace), flush_interval);
    let traces = spawn_trace_ingestion(dataspace, trace_collector);
    IngestionHandles { qos, traces }
}

/// Loop de ingestão de `Execution.Trace` → [`TraceCollector`].
pub fn spawn_trace_ingestion(
    dataspace: Arc<dds_dataspace::DataSpace>,
    collector: Arc<TraceCollector>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        use futures::StreamExt;
        let mut stream = Box::pin(dataspace.stream_execution_traces());
        while let Some(event) = stream.next().await {
            collector.ingest(&event);
        }
    })
}
