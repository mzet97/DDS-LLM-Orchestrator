//! Motor do Neuro-Fuzzy Cognitive Map (inferência).
//!
//! Corrige as limitações da versão FCM: a pertinência entra na inferência (L1),
//! os nós internos evoluem com realimentação real (L2) e os pesos causais vêm de
//! regras NFIS (L3). Porte de `neuro_fuzzy/nfcm.py` com índices fixos (rápido,
//! sem alocação no laço quente). Reproduz os números do artigo (Seção 8).

use crate::membership::{default_terms, fuzzify, GaussTerm, ALTO};

pub const N_METRICS: usize = 8;
pub const N_NODES: usize = 3; // h_pressure, h_health, h_stream
pub const N_PROFILES: usize = 5;

pub const METRICS: [&str; N_METRICS] = [
    "urgency",
    "deadline_pressure",
    "recent_latency",
    "agent_load",
    "error_rate",
    "historical_confidence",
    "estimated_complexity",
    "streaming_need",
];
pub const NODES: [&str; N_NODES] = ["h_pressure", "h_health", "h_stream"];
pub const PROFILES: [&str; N_PROFILES] = [
    "QoS_Critical",
    "QoS_Failover",
    "QoS_StreamLike",
    "QoS_LowCost",
    "QoS_Balanced",
];

#[inline]
fn sigmoid(z: f64, lam: f64) -> f64 {
    let z = (lam * z).clamp(-60.0, 60.0);
    1.0 / (1.0 + (-z).exp())
}

/// Contribuição fixa entrada(fuzzy)→nó interno: (métrica, termo, nó, peso).
#[derive(Clone, Copy)]
pub struct WxFixed {
    pub metric: usize,
    pub term: usize,
    pub node: usize,
    pub w: f64,
}

/// Regra NFIS: modula um peso causal pela pertinência. w_eff = w0·(1 + beta·μ).
#[derive(Clone, Copy)]
pub struct NfisRule {
    pub metric: usize,
    pub term: usize,
    pub node: usize,
    pub w0: f64,
    pub beta: f64,
}

/// Configuração do NFCM (pesos = inicialização por especialista; treináveis).
#[derive(Clone)]
pub struct NfcmConfig {
    pub terms: [GaussTerm; 3],
    pub wx_fixed: Vec<WxFixed>,
    pub nfis: Vec<NfisRule>,
    pub wh: Vec<(usize, usize, f64)>, // (origem, destino, peso)
    pub wo: [[f64; N_NODES]; N_PROFILES],
    pub bias: [f64; N_PROFILES],
    pub lam: f64,
    pub b_h: f64,
}

impl NfcmConfig {
    /// Inicialização por especialista (espelha `neuro_fuzzy/profile.py`).
    pub fn qos_default() -> Self {
        // (métrica, ALTO, nó, peso)
        let wx = vec![
            WxFixed {
                metric: 2,
                term: ALTO,
                node: 0,
                w: 0.90,
            }, // recent_latency→pressure
            WxFixed {
                metric: 3,
                term: ALTO,
                node: 0,
                w: 0.90,
            }, // agent_load→pressure
            WxFixed {
                metric: 0,
                term: ALTO,
                node: 0,
                w: 0.80,
            }, // urgency→pressure
            WxFixed {
                metric: 1,
                term: ALTO,
                node: 0,
                w: 0.60,
            }, // deadline_pressure→pressure
            WxFixed {
                metric: 5,
                term: ALTO,
                node: 1,
                w: 1.50,
            }, // hist_confidence→health
            WxFixed {
                metric: 2,
                term: ALTO,
                node: 1,
                w: -0.60,
            }, // recent_latency→health(−)
            WxFixed {
                metric: 7,
                term: ALTO,
                node: 2,
                w: 3.00,
            }, // streaming_need→stream
        ];
        let nfis = vec![NfisRule {
            metric: 4,
            term: ALTO,
            node: 1,
            w0: -0.40,
            beta: 0.50,
        }];
        let wh = vec![(0, 1, -0.30), (1, 0, -0.20)];
        let wo = [
            [1.6, 1.4, 0.0],   // Critical
            [1.2, -2.2, 0.0],  // Failover
            [0.0, 0.0, 2.4],   // StreamLike
            [-1.8, 0.6, -0.4], // LowCost
            [-0.5, 1.0, -0.2], // Balanced
        ];
        let bias = [-0.7, 0.7, -0.2, 0.5, 0.3];
        Self {
            terms: default_terms(),
            wx_fixed: wx,
            nfis,
            wh,
            wo,
            bias,
            lam: 2.5,
            b_h: -1.3,
        }
    }
}

/// Resultado de uma inferência.
#[derive(Clone)]
pub struct NfcmResult {
    pub memberships: [[f64; 3]; N_METRICS],
    /// (métrica, termo, nó, peso_efetivo) das regras NFIS neste estado.
    pub adjusted: Vec<(usize, usize, usize, f64)>,
    pub drives: [f64; N_NODES],
    pub h_final: [f64; N_NODES],
    pub iterations: usize,
    pub converged: bool,
    pub scores: [f64; N_PROFILES],
    pub logits: [f64; N_PROFILES],
    pub winner: usize,
    pub margin: f64,
}

impl NfcmResult {
    pub fn winner_name(&self) -> &'static str {
        PROFILES[self.winner]
    }
}

pub struct Nfcm {
    pub cfg: NfcmConfig,
}

impl Nfcm {
    pub fn new(cfg: NfcmConfig) -> Self {
        Self { cfg }
    }
    pub fn qos_default() -> Self {
        Self::new(NfcmConfig::qos_default())
    }

    pub fn infer(&self, metrics: &[f64; N_METRICS]) -> NfcmResult {
        let cfg = &self.cfg;
        // 1. fuzificação
        let mut mu = [[0.0; 3]; N_METRICS];
        for i in 0..N_METRICS {
            mu[i] = fuzzify(metrics[i], &cfg.terms);
        }
        // 2. drives (wx fixo + NFIS)
        let mut drives = [0.0f64; N_NODES];
        for c in &cfg.wx_fixed {
            drives[c.node] += c.w * mu[c.metric][c.term];
        }
        let mut adjusted = Vec::with_capacity(cfg.nfis.len());
        for r in &cfg.nfis {
            let m = mu[r.metric][r.term];
            let w_eff = r.w0 * (1.0 + r.beta * m);
            adjusted.push((r.metric, r.term, r.node, w_eff));
            drives[r.node] += w_eff * m;
        }
        // 3. dinâmica: itera h (realimentação real) até atrator
        let mut h = [0.0f64; N_NODES];
        let mut iterations = 0;
        let mut converged = false;
        for it in 1..=100 {
            let mut nxt = [0.0f64; N_NODES];
            for n in 0..N_NODES {
                let mut rec = 0.0;
                for &(s, d, w) in &cfg.wh {
                    if d == n {
                        rec += w * h[s];
                    }
                }
                nxt[n] = sigmoid(drives[n] + rec + cfg.b_h, cfg.lam);
            }
            let delta = (0..N_NODES)
                .map(|n| (nxt[n] - h[n]).abs())
                .fold(0.0, f64::max);
            h = nxt;
            iterations = it;
            if delta < 1e-4 {
                converged = true;
                break;
            }
        }
        // 4. decisão (softmax)
        let mut logits = [0.0f64; N_PROFILES];
        for (p, logit) in logits.iter_mut().enumerate() {
            let mut z = cfg.bias[p];
            for (n, &hn) in h.iter().enumerate() {
                z += cfg.wo[p][n] * hn;
            }
            *logit = z;
        }
        let maxl = logits.iter().cloned().fold(f64::MIN, f64::max);
        let mut exps = [0.0f64; N_PROFILES];
        let mut zsum = 0.0;
        for p in 0..N_PROFILES {
            exps[p] = (logits[p] - maxl).exp();
            zsum += exps[p];
        }
        let mut scores = [0.0f64; N_PROFILES];
        for (p, score) in scores.iter_mut().enumerate() {
            *score = exps[p] / zsum;
        }
        // vencedor + margem
        let mut winner = 0;
        for p in 1..N_PROFILES {
            if scores[p] > scores[winner] {
                winner = p;
            }
        }
        let mut second = f64::MIN;
        for (p, &s) in scores.iter().enumerate() {
            if p != winner && s > second {
                second = s;
            }
        }
        let margin = scores[winner] - second;

        NfcmResult {
            memberships: mu,
            adjusted,
            drives,
            h_final: h,
            iterations,
            converged,
            scores,
            logits,
            winner,
            margin,
        }
    }
}

// ── QosDecider (T-501) ─────────────────────────────────────────────────────

impl crate::decider::QosDecider for Nfcm {
    fn decide(&self, metrics: &crate::decider::QoSMetrics) -> crate::decider::QoSDecision {
        let arr = [
            metrics.urgency,
            metrics.deadline_pressure,
            metrics.recent_latency,
            metrics.agent_load,
            metrics.error_rate,
            metrics.historical_confidence,
            metrics.estimated_complexity,
            metrics.streaming_need,
        ];
        let r = self.infer(&arr);
        let profile = match r.winner {
            0 => crate::QoSProfile::Critical,
            1 => crate::QoSProfile::Failover,
            2 => crate::QoSProfile::StreamLike,
            3 => crate::QoSProfile::LowCost,
            _ => crate::QoSProfile::Balanced,
        };
        crate::decider::QoSDecision {
            profile,
            confidence: r.scores[r.winner],
            explanation: crate::explain_text(&r),
        }
    }

    fn name(&self) -> &str {
        "nfcm"
    }
}
