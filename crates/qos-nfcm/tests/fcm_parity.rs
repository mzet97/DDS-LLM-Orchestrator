//! Paridade FCM vs Python (`fcm_qos_manager/`) + divergência DHL (REQ-503).
//!
//! Valores esperados gerados pela execução real do Python
//! (`dump_fcm_expected.py`, 2026-07-18): 7 iterações, fixed_point em todos.

use qos_nfcm::decider::{QoSMetrics, QosDecider};
use qos_nfcm::fcm::{build_qos_fcm, decide_qos, FcmDecider, FcmDhlDecider, Termination};
use qos_nfcm::zadeh::ZadehDecider;
use qos_nfcm::QoSProfile;
use std::collections::HashMap;

const KEYS: [&str; 8] = [
    "urgency",
    "deadline_pressure",
    "recent_latency",
    "agent_load",
    "error_rate",
    "historical_confidence",
    "estimated_complexity",
    "streaming_need",
];

fn state(vals: &[f64; 8]) -> HashMap<String, f64> {
    KEYS.iter()
        .zip(vals.iter())
        .map(|(k, v)| (k.to_string(), *v))
        .collect()
}

fn qm(vals: &[f64; 8]) -> QoSMetrics {
    QoSMetrics {
        urgency: vals[0],
        deadline_pressure: vals[1],
        recent_latency: vals[2],
        agent_load: vals[3],
        error_rate: vals[4],
        historical_confidence: vals[5],
        estimated_complexity: vals[6],
        streaming_need: vals[7],
    }
}

struct Expected {
    winner: &'static str,
    iterations: usize,
    /// ativação por conceito de decisão (ordem Critical, Failover, StreamLike, LowCost, Balanced)
    activations: [f64; 5],
}

fn scenarios() -> Vec<(&'static str, [f64; 8], Expected)> {
    vec![
        (
            "ocioso",
            [0.10, 0.05, 0.15, 0.10, 0.05, 0.90, 0.20, 0.05],
            Expected {
                winner: "QoS_Balanced",
                iterations: 7,
                activations: [0.689270, 0.641363, 0.687905, 0.631579, 0.691988],
            },
        ),
        (
            "urgencia",
            [0.95, 0.90, 0.30, 0.35, 0.10, 0.85, 0.50, 0.10],
            Expected {
                winner: "QoS_Critical",
                iterations: 7,
                activations: [0.781922, 0.723892, 0.725138, 0.494971, 0.632337],
            },
        ),
        (
            "degradado",
            [0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10],
            Expected {
                winner: "QoS_Failover",
                iterations: 7,
                activations: [0.686536, 0.803532, 0.661916, 0.546456, 0.507469],
            },
        ),
        (
            "streaming",
            [0.50, 0.30, 0.40, 0.45, 0.10, 0.80, 0.40, 0.92],
            Expected {
                winner: "QoS_StreamLike",
                iterations: 7,
                activations: [0.715041, 0.700039, 0.765369, 0.527266, 0.640615],
            },
        ),
        (
            "lowcost",
            [0.05, 0.10, 0.20, 0.15, 0.05, 0.70, 0.05, 0.05],
            Expected {
                winner: "QoS_Balanced",
                iterations: 7,
                activations: [0.676836, 0.656119, 0.676836, 0.645828, 0.678932],
            },
        ),
        (
            "critico",
            [0.90, 0.95, 0.50, 0.40, 0.05, 0.80, 0.30, 0.20],
            Expected {
                winner: "QoS_Critical",
                iterations: 7,
                activations: [0.770358, 0.734957, 0.723892, 0.509968, 0.623188],
            },
        ),
    ]
}

#[test]
fn paridade_fcm_com_python() {
    let fcm = build_qos_fcm();
    let decision = [
        "QoS_Critical",
        "QoS_Failover",
        "QoS_StreamLike",
        "QoS_LowCost",
        "QoS_Balanced",
    ];

    for (name, vals, exp) in scenarios() {
        let metrics = state(&vals);
        let (winner, score, r) = decide_qos(&fcm, &metrics);
        assert_eq!(
            winner, exp.winner,
            "cenário {name}: vencedor diverge do Python"
        );
        assert_eq!(
            r.iterations, exp.iterations,
            "cenário {name}: iterações divergem"
        );
        assert_eq!(
            r.kind,
            Termination::FixedPoint,
            "cenário {name}: não convergiu"
        );

        for (i, c) in decision.iter().enumerate() {
            let got = r.final_state[*c];
            assert!(
                (got - exp.activations[i]).abs() < 1e-4,
                "cenário {name} {c}: ativação Rust {got:.6} vs Python {:.6}",
                exp.activations[i]
            );
        }
        assert!(
            (score - exp.activations[decision.iter().position(|c| *c == exp.winner).unwrap()])
                .abs()
                < 1e-4
        );
    }
}

#[test]
fn divergencia_fcm_vs_linear_em_lote_barato() {
    // Cenário "lote barato": o Zadeh linear escolhe LowCost, mas o FCM (com as
    // arestas ENTRE conceitos) escolhe Balanced — divergência estrutural, sem aprendizado.
    let lowcost = [0.05, 0.10, 0.20, 0.15, 0.05, 0.70, 0.05, 0.05];

    let zadeh = ZadehDecider::new();
    let fcm = FcmDecider::new();

    let dz = zadeh.decide(&qm(&lowcost));
    let df = fcm.decide(&qm(&lowcost));

    assert_eq!(
        dz.profile,
        QoSProfile::LowCost,
        "linear deveria escolher LowCost"
    );
    assert_eq!(
        df.profile,
        QoSProfile::Balanced,
        "FCM deveria divergir para Balanced"
    );
    assert_ne!(
        dz.profile, df.profile,
        "FCM deve divergir do linear em 'lote barato'"
    );
}

#[test]
fn dhl_converge_para_correlacao_observada() {
    // Série lo/hi: error_rate oscila 0.2↔0.9 (Δ≈±0.7) e a ativação de Failover
    // acompanha (Δ≈±0.15) → produto médio ≈ 0.7·0.15 ≈ 0.105.
    // O DHL deve CONVERGIR o peso error_rate→QoS_Failover para ~0.105
    // (fórmula de Kosko: w → média de Δi·Δj; validado empiricamente em probe).
    let dhl = FcmDhlDecider::new(0.1);
    let hi = [0.30, 0.30, 0.60, 0.30, 0.90, 0.40, 0.30, 0.10];
    let lo = [0.30, 0.30, 0.40, 0.30, 0.20, 0.40, 0.30, 0.10];

    let w0 = dhl.weight_of("error_rate", "QoS_Failover").unwrap();
    assert!((w0 - 0.25).abs() < 1e-9, "peso inicial 0.25");

    for _ in 0..6 {
        let _ = dhl.decide(&qm(&lo));
        let _ = dhl.decide(&qm(&hi));
    }

    let w1 = dhl.weight_of("error_rate", "QoS_Failover").unwrap();
    assert!(
        (w1 - 0.105).abs() < 0.03,
        "DHL deveria convergir para a correlação observada (~0.105): veio {w1}"
    );

    // A decisão no input fronteiriço muda de confiança coerentemente
    let x = [0.30, 0.30, 0.50, 0.30, 0.35, 0.50, 0.30, 0.10];
    let plain = FcmDecider::new().decide(&qm(&x));
    let learned = dhl.decide(&qm(&x));
    assert_eq!(
        plain.profile, learned.profile,
        "vencedor não deveria mudar aqui"
    );
    assert!(
        learned.confidence < plain.confidence,
        "com a aresta enfraquecida, a confiança de Failover deve cair: {} vs {}",
        learned.confidence,
        plain.confidence
    );
}

#[test]
fn dhl_taxa_decai_por_passo() {
    use qos_nfcm::fcm::DifferentialHebbianLearner;
    let mut l = DifferentialHebbianLearner::new(0.1, 0.98);
    let mut w = HashMap::from([(("a".to_string(), "b".to_string()), 0.0)]);
    let s0 = HashMap::from([("a".to_string(), 0.0), ("b".to_string(), 0.0)]);
    let s1 = HashMap::from([("a".to_string(), 1.0), ("b".to_string(), 1.0)]);

    l.update_step(&mut w, &s0, &s1);
    let step1 = w[&("a".to_string(), "b".to_string())];
    l.update_step(&mut w, &s0, &s1);
    let step2 = w[&("a".to_string(), "b".to_string())] - step1;

    // Δw1 = 0.1·(1·1 − 0) = 0.1; segundo passo usa c_1 = 0.098 E subtrai w corrente
    assert!((step1 - 0.1).abs() < 1e-9, "primeiro passo {step1}");
    assert!(step2 < step1, "taxa deve decair: {step1} vs {step2}");
}
