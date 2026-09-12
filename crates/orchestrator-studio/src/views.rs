//! Painéis do binário `studio`, um módulo por responsabilidade.

pub mod agents;
pub mod catalog;
pub mod dispatch;
pub mod inference;
pub mod node;
#[cfg(feature = "dds")]
pub mod topology;
