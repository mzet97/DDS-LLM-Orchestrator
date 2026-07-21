//! Treinamento supervisionado do NFCM (gradiente numérico, PARALELO com rayon).
//!
//! O gradiente por diferenças finitas é "embaraçosamente paralelo": a perda
//! perturbada de cada parâmetro é independente → `par_iter` distribui pelos 24
//! threads do Ryzen. Porte de `neuro_fuzzy/trainer.py` (que era serial em Python
//! por causa do GIL — aqui é multicore de verdade).

use rayon::prelude::*;

use crate::dataset::Sample;
use crate::nfcm::{Nfcm, NfcmConfig, N_NODES, N_PROFILES};

/// Restrições de sinal conhecidas em W_o: (perfil, nó, sinal_esperado).
const SIGN: [(usize, usize, f64); 3] = [
    (1, 1, -1.0), // Failover.h_health <= 0
    (2, 2, 1.0),  // StreamLike.h_stream >= 0
    (0, 0, 1.0),  // Critical.h_pressure >= 0
];

#[derive(Clone, Copy)]
pub struct TrainConfig {
    pub lr: f64,
    pub epochs: usize,
    pub fd_eps: f64,
    pub lambda_expert: f64,
    pub lambda_sparse: f64,
    pub lambda_sign: f64,
    pub train_membership: bool,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            lr: 0.2,
            epochs: 60,
            fd_eps: 1e-3,
            lambda_expert: 0.02,
            lambda_sparse: 0.001,
            lambda_sign: 0.5,
            train_membership: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Key {
    Beta(usize),
    Wo(usize, usize),
    Bias(usize),
    TermC(usize),
    TermR(usize),
}

fn keys(cfg: &NfcmConfig, train_membership: bool) -> Vec<Key> {
    let mut k = Vec::new();
    for i in 0..cfg.nfis.len() {
        k.push(Key::Beta(i));
    }
    for p in 0..N_PROFILES {
        for n in 0..N_NODES {
            k.push(Key::Wo(p, n));
        }
        k.push(Key::Bias(p));
    }
    if train_membership {
        for t in 0..3 {
            k.push(Key::TermC(t));
            k.push(Key::TermR(t));
        }
    }
    k
}

fn get(cfg: &NfcmConfig, k: Key) -> f64 {
    match k {
        Key::Beta(i) => cfg.nfis[i].beta,
        Key::Wo(p, n) => cfg.wo[p][n],
        Key::Bias(p) => cfg.bias[p],
        Key::TermC(t) => cfg.terms[t].c,
        Key::TermR(t) => cfg.terms[t].r,
    }
}

fn set(cfg: &mut NfcmConfig, k: Key, v: f64) {
    match k {
        Key::Beta(i) => cfg.nfis[i].beta = v,
        Key::Wo(p, n) => cfg.wo[p][n] = v,
        Key::Bias(p) => cfg.bias[p] = v,
        Key::TermC(t) => cfg.terms[t].c = v,
        Key::TermR(t) => cfg.terms[t].r = v,
    }
}

fn cross_entropy(cfg: &NfcmConfig, samples: &[Sample]) -> f64 {
    let nfcm = Nfcm::new(cfg.clone());
    let mut total = 0.0;
    let mut n = 0;
    for s in samples {
        if let Some(q) = s.q_star {
            let r = nfcm.infer(&s.x);
            total += -r.scores[q].max(1e-9).ln();
            n += 1;
        }
    }
    total / n.max(1) as f64
}

fn reg(cfg: &NfcmConfig, wo0: &[[f64; N_NODES]; N_PROFILES], tc: &TrainConfig) -> f64 {
    let (mut expert, mut sparse, mut sign) = (0.0, 0.0, 0.0);
    for (p, wo0_row) in wo0.iter().enumerate() {
        for (n, &wo0_pn) in wo0_row.iter().enumerate() {
            let w = cfg.wo[p][n];
            expert += (w - wo0_pn).powi(2);
            sparse += w.abs();
        }
    }
    for &(p, n, s) in &SIGN {
        let viol = (-s * cfg.wo[p][n]).max(0.0);
        sign += viol * viol;
    }
    tc.lambda_expert * expert + tc.lambda_sparse * sparse + tc.lambda_sign * sign
}

fn total_loss(
    cfg: &NfcmConfig,
    samples: &[Sample],
    wo0: &[[f64; N_NODES]; N_PROFILES],
    tc: &TrainConfig,
) -> f64 {
    cross_entropy(cfg, samples) + reg(cfg, wo0, tc)
}

pub fn accuracy(cfg: &NfcmConfig, samples: &[Sample]) -> f64 {
    let nfcm = Nfcm::new(cfg.clone());
    let mut hit = 0;
    let mut tot = 0;
    for s in samples {
        if let Some(q) = s.q_star {
            tot += 1;
            if nfcm.infer(&s.x).winner == q {
                hit += 1;
            }
        }
    }
    hit as f64 / tot.max(1) as f64
}

pub struct TrainHistory {
    pub train_loss: Vec<f64>,
    pub val_acc: Vec<f64>,
}

pub struct NfcmTrainer {
    pub cfg: NfcmConfig,
    tc: TrainConfig,
    keys: Vec<Key>,
    wo0: [[f64; N_NODES]; N_PROFILES],
}

impl NfcmTrainer {
    pub fn new(base: NfcmConfig, tc: TrainConfig) -> Self {
        let keys = keys(&base, tc.train_membership);
        let wo0 = base.wo;
        Self {
            cfg: base,
            tc,
            keys,
            wo0,
        }
    }

    pub fn fit(&mut self, train: &[Sample], val: Option<&[Sample]>) -> TrainHistory {
        let mut hist = TrainHistory {
            train_loss: Vec::new(),
            val_acc: Vec::new(),
        };
        for _ in 0..self.tc.epochs {
            let base: Vec<f64> = self.keys.iter().map(|&k| get(&self.cfg, k)).collect();
            let l0 = total_loss(&self.cfg, train, &self.wo0, &self.tc);
            // gradiente PARALELO (um parâmetro por thread)
            let grad: Vec<f64> = (0..self.keys.len())
                .into_par_iter()
                .map(|i| {
                    let mut c = self.cfg.clone();
                    set(&mut c, self.keys[i], base[i] + self.tc.fd_eps);
                    (total_loss(&c, train, &self.wo0, &self.tc) - l0) / self.tc.fd_eps
                })
                .collect();
            for (i, &k) in self.keys.iter().enumerate() {
                set(&mut self.cfg, k, base[i] - self.tc.lr * grad[i]);
            }
            hist.train_loss
                .push(total_loss(&self.cfg, train, &self.wo0, &self.tc));
            if let Some(v) = val {
                hist.val_acc.push(accuracy(&self.cfg, v));
            }
        }
        hist
    }
}
