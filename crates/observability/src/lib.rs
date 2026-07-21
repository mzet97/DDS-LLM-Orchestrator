//! # observability
//!
//! Stack de observabilidade do orquestrador DDS-LLM: porte dos componentes
//! Python `observability/`, `qos_collector/`, `trace_collector/` e `metrics/`
//! para Rust (10ª crate do workspace de migração).
//!
//! ## Componentes e fontes
//! | Módulo | Fonte Python | Papel |
//! |---|---|---|
//! | [`events`] | `observability/events.py` | schema unificado de eventos |
//! | [`sink`] | `observability/file_sink.py` | trait `EventSink` + sink JSONL |
//! | [`qos_store`] | `qos_collector/qos_store.py` | store em memória (SQL → `DashMap`) |
//! | [`qos_collector`] | `qos_collector/main.py` | agregador de métricas/violações/discovery |
//! | [`trace_collector`] | `trace_collector/trace_collector.py` | agregador de `Execution.Trace` |
//! | [`trackers`] | `metrics/{token_counter,rtt_tracker,cost_tracker}.py` | tokens/RTT/custo com atômicos |
//! | [`dds`] | `dds_backend` (`subscribe_qos_*`, `subscribe_execution_traces`) | tópicos + loops de ingestão (feature `dds`) |
//!
//! ## Decisões de porte
//! - **Sem sqlx/diesel**: o store PostgreSQL (`qos_store.py` + `schema.sql`) vira
//!   `DashMap` em memória + snapshot JSONL. Postgres fica como follow-up
//!   documentado em [`qos_store`].
//! - **Atômicos em vez de locks**: os contadores do Python tinham o bug C3
//!   (read-modify-write sem lock entre workers). Aqui são `AtomicU64` +
//!   `DashMap` — corretos por construção, sem lock global.
//! - Nomes de tópicos/tipos/eventos idênticos aos do Python/IDL (interop).
#![deny(warnings)]

pub mod events;
pub mod qos_collector;
pub mod qos_store;
pub mod sink;
pub mod trace_collector;
pub mod trackers;

#[cfg(feature = "dds")]
pub mod dds;

pub use events::{EventType, ObservabilityEvent};
pub use qos_collector::{CollectorStats, QosCollector};
pub use qos_store::QosStore;
pub use sink::{EventSink, FileEventSink, SinkError};
pub use trace_collector::TraceCollector;
pub use trackers::{CostTracker, ErrorTracker, RttTracker, TokenCounter};
