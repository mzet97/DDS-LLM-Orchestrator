//! # qos-nfcm
//!
//! Neuro-Fuzzy Cognitive Map para seleção adaptativa e interpretável de QoS.
//! Porte Rust de `src/orchestrator/neuro_fuzzy/` (Python). Ganhos de Rust:
//! sem GIL, inferência sem alocação no laço quente, e **treino paralelo (rayon)**
//! nos 24 threads do Ryzen — o que em Python era serial por causa do GIL.
//!
//! Reproduz os números do artigo (Seção 8) e discrimina os 4 cenários canônicos.

pub mod baselines;
pub mod dataset;
pub mod decider;
pub mod fcm;
pub mod membership;
pub mod nfcm;
pub mod stability;
pub mod trainer;
pub mod utility;
pub mod zadeh;

pub use baselines::{FixedRulesDecider, MamdaniDecider, SwUcbDecider, Ucb1Decider};
pub use decider::{QoSDecision, QoSMetrics, QosDecider, StaticDecider};
pub use fcm::{FcmDecider, FcmDhlDecider};
pub use nfcm::{Nfcm, NfcmConfig, NfcmResult, METRICS, NODES, PROFILES};
pub use zadeh::ZadehDecider;

/// Perfis QoS disponíveis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QoSProfile {
    Critical,
    Failover,
    StreamLike,
    LowCost,
    Balanced,
}

/// Explicação interpretável da decisão (trilha causal resumida).
pub fn explain_text(r: &NfcmResult) -> String {
    // pertinência dominante por métrica
    let mut dom: Vec<(usize, usize, f64)> = (0..nfcm::N_METRICS)
        .map(|m| {
            let (t, mu) = (0..3)
                .map(|t| (t, r.memberships[m][t]))
                .fold((0usize, f64::MIN), |a, b| if b.1 > a.1 { b } else { a });
            (m, t, mu)
        })
        .collect();
    dom.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let terms = ["baixo", "medio", "alto"];
    let tops: Vec<String> = dom
        .iter()
        .take(3)
        .map(|(m, t, mu)| format!("{} é '{}' (μ={:.2})", METRICS[*m], terms[*t], mu))
        .collect();
    let nfis: Vec<String> = r
        .adjusted
        .iter()
        .map(|(m, _t, n, w)| format!("{}→{} ajustado p/ {:.3}", METRICS[*m], NODES[*n], w))
        .collect();
    format!(
        "{} selecionado (confiança {:.3}, margem {:.3}). Dominantes: {}. NFIS: {}.",
        r.winner_name(),
        r.scores[r.winner],
        r.margin,
        tops.join("; "),
        nfis.join("; ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{split, synthetic_dataset, CANONICAL};
    use crate::trainer::{accuracy, NfcmTrainer, TrainConfig};

    #[test]
    fn discrimina_os_quatro_cenarios() {
        let nfcm = Nfcm::qos_default();
        for (x, expected) in CANONICAL.iter() {
            let r = nfcm.infer(x);
            assert_eq!(
                r.winner,
                *expected,
                "cenário {:?} -> {}",
                x,
                r.winner_name()
            );
        }
    }

    #[test]
    fn reproduz_numeros_do_artigo_degradado() {
        let nfcm = Nfcm::qos_default();
        // cenário degradado (mesmo do exemplo numérico da Seção 8)
        let x = [0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10];
        let r = nfcm.infer(&x);
        // μ_alto(error_rate=0.90) ≈ 0.923
        assert!((r.memberships[4][2] - 0.923).abs() < 5e-3);
        // peso NFIS ajustado ≈ -0.585
        assert!(
            (r.adjusted[0].3 - (-0.585)).abs() < 5e-3,
            "w={}",
            r.adjusted[0].3
        );
        // saúde ≈ 0.002, pressão ≈ 0.712
        assert!(r.h_final[1] < 0.02, "h_health={}", r.h_final[1]);
        assert!(
            (r.h_final[0] - 0.712).abs() < 0.02,
            "h_pressure={}",
            r.h_final[0]
        );
        // vence Failover (idx 1) com softmax ≈ 0.551, margem ≈ 0.369
        assert_eq!(r.winner, 1);
        assert!((r.scores[1] - 0.551).abs() < 1e-2, "score={}", r.scores[1]);
        assert!((r.margin - 0.369).abs() < 1e-2, "margin={}", r.margin);
        assert!(r.converged);
    }

    #[test]
    fn explicacao_menciona_vencedor() {
        let r = Nfcm::qos_default().infer(&CANONICAL[2].0);
        let txt = explain_text(&r);
        assert!(txt.contains("QoS_Failover") && txt.contains("μ="));
    }

    #[test]
    fn trainer_paralelo_reduz_perda_e_melhora_acuracia() {
        let ds = synthetic_dataset(25, 0.06, 3);
        let (tr, va, te) = split(&ds, 0.6, 0.2);
        let mut cfg = NfcmConfig::qos_default();
        // degrada para haver o que aprender
        cfg.nfis[0].beta = 0.0;
        cfg.wo[1][1] = -0.5; // Failover.h_health
        let acc0 = accuracy(&cfg, &te);

        let mut trainer = NfcmTrainer::new(
            cfg,
            TrainConfig {
                lr: 0.3,
                epochs: 25,
                ..Default::default()
            },
        );
        let hist = trainer.fit(&tr, Some(&va));

        assert!(hist.train_loss.last().unwrap() < hist.train_loss.first().unwrap());
        let acc1 = accuracy(&trainer.cfg, &te);
        assert!(acc1 >= acc0);
        assert!(acc1 >= 0.7, "acc final {}", acc1);
    }

    #[test]
    fn treina_pertinencias_quando_habilitado() {
        let ds = synthetic_dataset(20, 0.06, 7);
        let (tr, va, _) = split(&ds, 0.6, 0.2);
        let cfg = NfcmConfig::qos_default();
        let c0: Vec<f64> = cfg.terms.iter().map(|t| t.c).collect();
        let mut trainer = NfcmTrainer::new(
            cfg,
            TrainConfig {
                lr: 0.2,
                epochs: 12,
                train_membership: true,
                ..Default::default()
            },
        );
        trainer.fit(&tr, Some(&va));
        let moved = (0..3).any(|i| (trainer.cfg.terms[i].c - c0[i]).abs() > 1e-6);
        assert!(moved, "centros de pertinência deveriam ter sido ajustados");
        assert!(trainer.cfg.terms.iter().all(|t| t.sigma() > 0.0));
    }
}
