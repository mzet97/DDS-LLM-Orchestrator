//! Ownership strength por papel (Fase 2.2, REQ-006).
//!
//! Valores validados no Python: cliente=10, agente=100, orquestrador=200.
//! O middleware DDS arbitra o claim; strengths maiores vencem.

/// Strength do cliente DDS (submete tasks).
pub const STRENGTH_CLIENT: i32 = 10;
/// Strength do agente (claim de tasks PENDING).
pub const STRENGTH_AGENT: i32 = 100;
/// Strength do orquestrador (failover/reaper vence agentes).
pub const STRENGTH_ORCHESTRATOR: i32 = 200;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_strengths_match_fase_2_2() {
        assert_eq!(STRENGTH_CLIENT, 10);
        assert_eq!(STRENGTH_AGENT, 100);
        assert_eq!(STRENGTH_ORCHESTRATOR, 200);
        const { assert!(STRENGTH_CLIENT < STRENGTH_AGENT) };
        const { assert!(STRENGTH_AGENT < STRENGTH_ORCHESTRATOR) };
    }
}
