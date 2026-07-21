//! # benchmarks
//!
//! Geração de carga E1–E5/OP1–OP4 + coleta JSONL — porte de `benchmarks/` e
//! `benchmarks/qualificacao/` (Python) para Rust (14ª crate do workspace).
//!
//! ## Componentes e fontes
//! | Módulo | Fonte Python | Papel |
//! |---|---|---|
//! | [`rng`] | `np.random.default_rng` | RNG determinístico (xoshiro256**) |
//! | [`regimes`] | `real_workload_driver.py::REGIMES` | leve/moderada/pesada (artigo §6.2) |
//! | [`generator`] | `real_workload_driver.py` | Poisson + bursts + prompts lognormal |
//! | [`scenarios`] | `qualificacao/scripts/run_*.py`, `E3_priority.py` | registry E1–E5/OP1–OP4 |
//! | [`metrics`] | `instrumentation/trace_schema.py::RequestRecord` | JSONL por braço/cenário |
//! | [`driver`] (feature `dds`) | `real_workload_driver.py`, `E3_priority.py` | publica tasks DDS e coleta |
//!
//! ## Fronteiras (não portado — e por quê)
//! - **Análise estatística** (Friedman, mixed models, índice de Jain, plots):
//!   permanece em `benchmarks/qualificacao/analysis/` (Python/pandas). Este
//!   crate PRODUZ o JSONL no schema que ela consome.
//! - **Baselines de QoS** (FixedRules/Mamdani/UCB1/SW-UCB): vivem em
//!   `qos-nfcm::baselines` (porte com testes de discriminação — WF-7),
//!   re-exportados aqui por conveniência.
//! - **Injeção de falha** (OP3): o kill do agente é operacional (fora do
//!   driver); o driver mede a recuperação.
#![deny(warnings)]

pub mod generator;
pub mod metrics;
pub mod regimes;
pub mod rng;
pub mod scenarios;

#[cfg(feature = "dds")]
pub mod driver;

pub use metrics::{JsonlWriter, RequestRecord, RequestStatus};
pub use regimes::{regime, WorkloadConfig, LEVE, MODERADA, PESADA, REGIMES};
pub use scenarios::{get as get_scenario, registry, Scenario, WorkloadPattern};

// Re-export dos braços de QoS (porte verificado em WF-7, com paridade).
pub use qos_nfcm::baselines::{FixedRulesDecider, MamdaniDecider, SwUcbDecider, Ucb1Decider};
pub use qos_nfcm::decider::{QoSDecision, QoSMetrics, QosDecider};

#[cfg(feature = "dds")]
pub use driver::{available_arms, BenchError, BenchmarkDriver, DriverConfig, RunSummary};
