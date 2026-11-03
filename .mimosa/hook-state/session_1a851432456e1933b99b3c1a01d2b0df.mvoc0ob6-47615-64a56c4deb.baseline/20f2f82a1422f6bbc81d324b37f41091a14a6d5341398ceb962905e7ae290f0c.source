//! Robustez do decisor NFCM: entradas não-finitas nunca derrubam o caller e
//! NÃO diferenciam os braços (REQ/T-820-08 — paridade com
//! `zadeh_robustness.rs` e com o fallback de `fcm.rs`).

use qos_nfcm::decider::{QoSMetrics, QosDecider};
use qos_nfcm::nfcm::Nfcm;
use qos_nfcm::QoSProfile;

#[test]
fn decide_com_metrica_nan_retorna_fallback_sem_panic() {
    let decider = Nfcm::qos_default();
    let metrics = QoSMetrics {
        recent_latency: f64::NAN,
        ..QoSMetrics::default()
    };

    let decision = decider.decide(&metrics);

    assert_eq!(decision.profile, QoSProfile::Balanced);
    assert_eq!(decision.confidence, 0.0);
    assert_eq!(decision.runner_up, 0.0);
    assert!(
        !decision.converged,
        "fallback deve sinalizar converged=false para o loop manter o perfil atual"
    );
    assert!(decision.explanation.contains("não finita"));
}

#[test]
fn decide_com_metrica_infinita_retorna_fallback_sem_panic() {
    let decider = Nfcm::qos_default();
    let metrics = QoSMetrics {
        error_rate: f64::INFINITY,
        ..QoSMetrics::default()
    };

    let decision = decider.decide(&metrics);

    assert_eq!(decision.profile, QoSProfile::Balanced);
    assert!(!decision.converged);
}

#[test]
fn decide_com_todas_metricas_finitas_nao_muda_contrato() {
    let decider = Nfcm::qos_default();
    let decision = decider.decide(&QoSMetrics::default());

    // Entrada sã: nada de fallback (converged sinaliza a inferência real).
    assert_ne!(decision.explanation, "nfcm: fallback (métrica não finita)");
    assert!(decision.confidence.is_finite());
    assert!(decision.runner_up.is_finite());
}
