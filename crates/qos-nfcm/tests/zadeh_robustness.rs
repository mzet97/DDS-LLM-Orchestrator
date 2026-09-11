//! Robustez do decisor Zadeh: entradas não-finitas nunca derrubam o caller.
//! (H1 da revisão `src/rust`: `decide` roda em `tokio::spawn` detached no
//! control-loop — panic ali congela a adaptação QoS no último perfil.)

use qos_nfcm::decider::{QoSMetrics, QosDecider};
use qos_nfcm::zadeh::{FuzzyNumber, ZadehDecider, ZadehSelector};
use qos_nfcm::QoSProfile;
use std::collections::HashMap;

#[test]
fn from_crisp_rejeita_nan_em_vez_de_panicar() {
    assert!(FuzzyNumber::from_crisp(f64::NAN).is_err());
    assert!(FuzzyNumber::from_crisp(f64::INFINITY).is_err());
    assert!(FuzzyNumber::from_crisp(0.5).is_ok());
}

#[test]
fn new_rejeita_cuts_nao_finitos() {
    let cuts = vec![
        qos_nfcm::zadeh::AlphaCut {
            alpha: 0.0,
            lower: f64::NAN,
            upper: f64::NAN,
        },
        qos_nfcm::zadeh::AlphaCut {
            alpha: 1.0,
            lower: 0.5,
            upper: 0.5,
        },
    ];
    assert!(FuzzyNumber::new(cuts).is_err());
}

#[test]
fn decide_com_metrica_nan_retorna_fallback_sem_panic() {
    let decider = ZadehDecider::new();
    let metrics = QoSMetrics {
        recent_latency: f64::NAN,
        ..QoSMetrics::default()
    };

    let decision = decider.decide(&metrics);

    assert_eq!(decision.profile, QoSProfile::Balanced);
    assert!(
        !decision.converged,
        "fallback deve sinalizar converged=false para o loop manter o perfil atual"
    );
}

#[test]
fn decide_com_metrica_infinita_retorna_fallback_sem_panic() {
    let decider = ZadehDecider::new();
    let metrics = QoSMetrics {
        error_rate: f64::INFINITY,
        ..QoSMetrics::default()
    };

    let decision = decider.decide(&metrics);

    assert_eq!(decision.profile, QoSProfile::Balanced);
    assert!(!decision.converged);
}

#[test]
fn decide_valido_continua_convergido() {
    let decider = ZadehDecider::new();
    let decision = decider.decide(&QoSMetrics::default());
    assert!(decision.converged);
    assert!(decision.confidence.is_finite());
}

#[test]
fn select_vazio_ou_degenerado_nao_panica() {
    // Seletor padrão tem perfis; com entradas vazias usa o default Python (0.5).
    let sel = ZadehSelector::new();
    let empty: HashMap<&'static str, FuzzyNumber> = HashMap::new();
    let best = sel
        .select(&empty, true)
        .expect("default cobre entradas ausentes");
    assert!(best.centroid.is_finite());
}
