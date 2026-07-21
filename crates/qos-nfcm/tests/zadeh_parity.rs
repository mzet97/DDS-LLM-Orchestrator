//! Paridade Zadeh vs Python (`fuzzy_qos_manager/qos_selector.py`) — REQ-502.
//!
//! Valores esperados gerados pela execução real do Python
//! (`scripts/dump_zadeh_expected.py`, 2026-07-18). Entradas crisp → a
//! avaliação por extensão é determinística; exigimos match exato (1e-9).

use qos_nfcm::zadeh::{AlphaCut, ExtensionPrincipleEvaluator, FuzzyNumber, ZadehSelector};
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

fn inputs(vals: &[f64; 8]) -> HashMap<&'static str, FuzzyNumber> {
    KEYS.iter()
        .zip(vals.iter())
        .map(|(k, v)| (*k, FuzzyNumber::from_crisp(*v)))
        .collect()
}

struct Expected {
    winner: QoSProfile,
    /// centroid por perfil na ordem [Critical, Failover, StreamLike, LowCost, Balanced]
    centroids: [f64; 5],
}

fn scenarios() -> Vec<(&'static str, [f64; 8], Expected)> {
    vec![
        (
            "ocioso",
            [0.10, 0.05, 0.15, 0.10, 0.05, 0.90, 0.20, 0.05],
            Expected {
                winner: QoSProfile::Balanced,
                centroids: [0.407500, 0.090000, 0.377500, 0.807500, 0.842500],
            },
        ),
        (
            "urgencia",
            [0.95, 0.90, 0.30, 0.35, 0.10, 0.85, 0.50, 0.10],
            Expected {
                winner: QoSProfile::Critical,
                centroids: [0.795000, 0.390000, 0.520000, 0.385000, 0.635000],
            },
        ),
        (
            "degradado",
            [0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10],
            Expected {
                winner: QoSProfile::Failover,
                centroids: [0.397500, 0.755000, 0.285000, 0.540000, 0.247500],
            },
        ),
        (
            "streaming",
            [0.50, 0.30, 0.40, 0.45, 0.10, 0.80, 0.40, 0.92],
            Expected {
                winner: QoSProfile::StreamLike,
                centroids: [0.505000, 0.297500, 0.692000, 0.482000, 0.662500],
            },
        ),
        (
            "lowcost",
            [0.05, 0.10, 0.20, 0.15, 0.05, 0.70, 0.05, 0.05],
            Expected {
                winner: QoSProfile::LowCost,
                centroids: [0.362500, 0.140000, 0.337500, 0.855000, 0.795000],
            },
        ),
        (
            "critico",
            [0.90, 0.95, 0.50, 0.40, 0.05, 0.80, 0.30, 0.20],
            Expected {
                winner: QoSProfile::Critical,
                centroids: [0.740000, 0.435000, 0.515000, 0.430000, 0.605000],
            },
        ),
    ]
}

#[test]
fn paridade_selecao_e_centroid_com_o_python() {
    let sel = ZadehSelector::new();
    let order = [
        QoSProfile::Critical,
        QoSProfile::Failover,
        QoSProfile::StreamLike,
        QoSProfile::LowCost,
        QoSProfile::Balanced,
    ];

    for (name, metrics, exp) in scenarios() {
        let inp = inputs(&metrics);
        let best = sel.select(&inp, true);
        assert_eq!(
            best.profile, exp.winner,
            "cenário {name}: vencedor diverge do Python"
        );

        let all = sel.evaluate_all(&inp);
        for (i, p) in order.iter().enumerate() {
            let got = all
                .iter()
                .find(|s| &s.profile == p)
                .expect("perfil presente");
            assert!(
                (got.centroid - exp.centroids[i]).abs() < 1e-9,
                "cenário {name} perfil {p:?}: centroid Rust {:.6} vs Python {:.6}",
                got.centroid,
                exp.centroids[i]
            );
            // com entradas crisp, lower_08 == centroid (intervalo degenerado)
            assert!((got.lower_08 - got.centroid).abs() < 1e-9);
        }
    }
}

// ── Unidades do FuzzyNumber (valores computados do algoritmo do Python) ────

#[test]
fn fuzzy_number_interpolacao_linear() {
    let n = FuzzyNumber::new(vec![
        AlphaCut {
            alpha: 0.0,
            lower: 0.0,
            upper: 1.0,
        },
        AlphaCut {
            alpha: 1.0,
            lower: 0.4,
            upper: 0.6,
        },
    ])
    .unwrap();
    // np.interp em alpha=0.5: lower 0.2, upper 0.8
    assert!((n.lower_bound(0.5).unwrap() - 0.2).abs() < 1e-12);
    assert!((n.upper_bound(0.5).unwrap() - 0.8).abs() < 1e-12);
    // fora da faixa → extremo
    assert!((n.lower_bound(0.0).unwrap() - 0.0).abs() < 1e-12);
    assert!((n.upper_bound(1.0).unwrap() - 0.6).abs() < 1e-12);
}

#[test]
fn fuzzy_number_centroid_trapezio() {
    // Suporte [0,1] em alpha=0, núcleo [0.4,0.6] em alpha=1 (mesma forma acima):
    // 2 cortes → área do trapézio = ((0+1)/2 + (0.4+0.6)/2)/2 = 0.5
    // momento = (1-0)*((1+0)/2)*1 = 0.5 → centroid = 0.5/... verificação direta:
    let n = FuzzyNumber::new(vec![
        AlphaCut {
            alpha: 0.0,
            lower: 0.0,
            upper: 1.0,
        },
        AlphaCut {
            alpha: 1.0,
            lower: 0.4,
            upper: 0.6,
        },
    ])
    .unwrap();
    let c = n.centroid();
    // mom = ((1.0-0.0) * (1.0+0.0)/2) * 1 = 0.5; area = 1.0 - 0.5? — o algoritmo usa
    // avg_lower/avg_upper por segmento: area=0.5, moment=0.25 → centroid=0.5
    assert!((c - 0.5).abs() < 1e-9, "centroid {c}");
}

#[test]
fn fuzzy_number_canonical_extrapola_extremos() {
    // Sem α=0/α=1: extrapola dos cortes adjacentes
    let n = FuzzyNumber::canonical(vec![
        AlphaCut {
            alpha: 0.5,
            lower: 0.3,
            upper: 0.7,
        },
        AlphaCut {
            alpha: 0.75,
            lower: 0.4,
            upper: 0.6,
        },
    ])
    .unwrap();
    let (sl, su) = n.support();
    let (cl, cu) = n.core();
    // alpha=0: t = -0.5/0.25 = -2 → lower0 = 0.3 + (-2)*(0.4-0.3) = 0.1; upper0 = 0.7 + (-2)*(0.6-0.7) = 0.9
    assert!(
        (sl - 0.1).abs() < 1e-9 && (su - 0.9).abs() < 1e-9,
        "support {sl},{su}"
    );
    // alpha=1: t = (1-0.5)/(0.75-0.5) = 2 → lower1 = 0.3 + 2*(0.4-0.3) = 0.5; upper1 = 0.7 + 2*(0.6-0.7) = 0.5
    // (conferido contra o Python: core = (0.5, 0.5))
    assert!(
        (cl - 0.5).abs() < 1e-9 && (cu - 0.5).abs() < 1e-9,
        "core {cl},{cu}"
    );
}

#[test]
fn extensao_vertices_min_max() {
    // f(x, y) = x - y: min = lower_x - upper_y; max = upper_x - lower_y (clamp [0,1])
    let x = FuzzyNumber::from_interval(0.5, 0.8);
    let y = FuzzyNumber::from_interval(0.1, 0.4);
    let eval = ExtensionPrincipleEvaluator::new(|p: &[f64]| p[0] - p[1], vec![0.0, 1.0]);
    let out = eval.evaluate(&[x, y]).unwrap();
    // min = 0.5-0.4 = 0.1; max = 0.8-0.1 = 0.7
    let (l, u) = out.support();
    assert!((l - 0.1).abs() < 1e-9);
    assert!((u - 0.7).abs() < 1e-9);
}

#[test]
fn extensao_aninhamento_dos_cuts() {
    // Função que não é monotônica por α: garante aninhamento mesmo assim
    let x = FuzzyNumber::triangular(0.0, 0.5, 1.0).unwrap();
    let eval =
        ExtensionPrincipleEvaluator::new(|p: &[f64]| (p[0] - 0.5).abs(), vec![0.0, 0.5, 1.0]);
    let out = eval.evaluate(&[x]).unwrap();
    let mut prev_upper = f64::INFINITY;
    for ac in [
        out.support().1,
        out.lower_bound(0.5)
            .unwrap()
            .max(out.upper_bound(0.5).unwrap()),
        out.core().1,
    ] {
        assert!(ac <= prev_upper + 1e-9, "aninhamento violado");
        prev_upper = ac;
    }
}
