#![cfg(feature = "dds")]
//! Observação DDS contra o domínio vivo (sem simulação de middleware).

/// Smoke no domínio real: só roda com `STUDIO_LIVE_DDS=1`.
/// Agentes vivos publicam heartbeats — ao menos um deve aparecer.
#[test]
fn live_domain42_shows_agents_when_requested() {
    if std::env::var("STUDIO_LIVE_DDS").is_err() {
        return;
    }
    let snapshot = orchestrator_studio::dds_observe::observe(42, std::time::Duration::from_secs(5))
        .expect("dominio 42 deve ser observavel");
    eprintln!(
        "fio vivo: {} agentes, {} tools, {} metricas, {} descobertas",
        snapshot.agents.len(),
        snapshot.tools.len(),
        snapshot.metrics.len(),
        snapshot.discoveries.len()
    );
    assert!(
        !snapshot.agents.is_empty(),
        "domínio vivo tem agentes com heartbeat"
    );
}
