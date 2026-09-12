//! Painéis do binário `studio`, um módulo por responsabilidade.

pub mod agent_editor;
pub mod agents;
pub mod catalog;
pub mod dispatch;
pub mod environments;
pub mod inference;
pub mod launch;
pub mod machines;
pub mod models;
pub mod node;
pub mod orchestrators;
pub mod overview;
pub mod services;
pub mod shared_catalog;
pub mod ssh;
pub mod tools;
#[cfg(feature = "dds")]
pub mod topology;
