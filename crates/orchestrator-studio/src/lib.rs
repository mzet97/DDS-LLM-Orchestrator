//! # orchestrator-studio
//!
//! Desktop nativo do DDS Orchestrator Studio (fase 800, P1). A lógica de
//! apresentação vive em [`state::AppState`] — pura e testada sem janela. O
//! binário `studio` é só a casca eframe que a renderiza (G-01 parcial:
//! build desktop nativo; somente leitura real, sem dados fictícios).

pub mod agent_defs;
pub mod agents;
pub mod catalog_remote;
#[cfg(feature = "dds")]
pub mod dds_observe;
pub mod design;
pub mod gallery;
pub mod inference;
pub mod launch;
pub mod machines;
pub mod models;
pub mod nodes;
pub mod orchestrators;
pub mod origin;
pub mod overview;
pub mod services;
pub mod shell;
pub mod ssh_session;
pub mod ssh_trust;
pub mod state;
pub mod workload;
