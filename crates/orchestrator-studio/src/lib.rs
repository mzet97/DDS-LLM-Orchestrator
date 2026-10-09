//! # orchestrator-studio
//!
//! Desktop nativo do DDS Orchestrator Studio (fase 800, P1). A lógica de
//! apresentação vive em [`state::AppState`] — pura e testada sem janela. O
//! binário `studio` é só a casca eframe que a renderiza (G-01 parcial:
//! build desktop nativo; somente leitura real, sem dados fictícios).

pub mod agents;
pub mod catalog_remote;
#[cfg(feature = "dds")]
pub mod dds_observe;
pub mod discovery;
pub mod inference;
/// Máquinas (REQ/T-840-03): registro multi-host no catálogo compartilhado
/// com sonda `/version` por nó — segredo só em memória (RNF-04).
pub mod kit;
pub mod launch;
pub mod machines;
pub mod models;
pub mod origin;
pub mod overview;
pub mod panel_header;
pub mod protected;
pub mod runner;
pub mod services;
pub mod state;
pub mod studio_log;
pub mod theme;
pub mod views;
pub mod workflow;
pub mod workload;
