//! # workflow-engine — domínio da fase 900 (T-900-M2-02)
//!
//! M2: **validator de decisão observador** — a decisão do Laya é DADO NÃO
//! CONFIÁVEL (D10) e só ganha efeito após validação Rust. Este crate é a
//! biblioteca de domínio (pura, sem DDS — testável com mocks do dds-contract);
//! o observador de processos reais é o bin `wf-observe` (feature `dds`).
//!
//! Marcos futuros: catálogo de capabilities/claim (M3), mapper de QoS (M4),
//! planner/DAG/materializer/journal (M5) — nada aqui publica Tasks.

pub mod capability_catalog;
pub mod dag;
pub mod decision;
pub mod eligibility;
pub mod engine;
pub mod journal;
#[cfg(feature = "dds")]
pub mod planner_http;
pub mod qos_mapper;
pub mod runtime;
pub mod task_id;

pub use capability_catalog::{catalog_v1, Catalog};
pub use decision::{DecisionPolicy, RejectReason, Verdict};
pub use eligibility::{is_eligible, validate_announcement, NotEligible};
