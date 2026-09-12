//! # orchestrator-studio
//!
//! Desktop nativo do DDS Orchestrator Studio (fase 800, P1). A lógica de
//! apresentação vive em [`state::AppState`] — pura e testada sem janela. O
//! binário `studio` é só a casca eframe que a renderiza (G-01 parcial:
//! build desktop nativo; somente leitura real, sem dados fictícios).

pub mod agents;
#[cfg(feature = "dds")]
pub mod dds_observe;
pub mod inference;
pub mod origin;
pub mod state;
pub mod workload;
