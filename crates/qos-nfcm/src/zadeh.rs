//! Seletor Zadeh — números fuzzy (α-cuts) + Princípio de Extensão (REQ-502).
//!
//! Porte fiel de `fuzzy_qos_manager/` (Python):
//! - `fuzzy_number.py` → [`FuzzyNumber`] (α-cuts, interpolação, centróide)
//! - `extension_principle.py` → [`ExtensionPrincipleEvaluator`] (vértices 2^n,
//!   clamp [0,1], aninhamento dos cuts)
//! - `qos_selector.py` → [`ZadehSelector`] (pesos canônicos por perfil;
//!   peso negativo ⇒ `|w|·(1−val)`; seleção conservadora por (lower_0.8, centroid))
//!
//! `ZadehDecider` é a fachada `QosDecider` (métricas crisp → inputs fuzzy).

use crate::decider::{QoSDecision, QoSMetrics, QosDecider};
use crate::QoSProfile;
use std::collections::HashMap;

// ── Erros ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum FuzzyError {
    #[error("FuzzyNumber precisa de pelo menos um alpha-cut")]
    Empty,
    #[error("alpha duplicado: {0}")]
    DuplicateAlpha(f64),
    #[error("lower não monotônico crescente em alpha={0}")]
    LowerNotMonotonic(f64),
    #[error("upper não monotônico decrescente em alpha={0}")]
    UpperNotMonotonic(f64),
    #[error("alpha fora de [0,1]: {0}")]
    AlphaOutOfRange(f64),
    #[error("triangular requer a <= b <= c")]
    BadTriangular,
}

// ── AlphaCut / FuzzyNumber ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaCut {
    pub alpha: f64,
    pub lower: f64,
    pub upper: f64,
}

/// Número fuzzy discreto por α-cuts (monotônicos: lower cresce, upper decresce).
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyNumber {
    cuts: Vec<AlphaCut>,
}

impl FuzzyNumber {
    pub fn new(mut cuts: Vec<AlphaCut>) -> Result<Self, FuzzyError> {
        if cuts.is_empty() {
            return Err(FuzzyError::Empty);
        }
        cuts.sort_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap());
        {
            let mut seen = std::collections::HashSet::new();
            for ac in &cuts {
                let key = (ac.alpha * 1e12) as i64;
                if !seen.insert(key) {
                    return Err(FuzzyError::DuplicateAlpha(ac.alpha));
                }
            }
        }
        let n = Self { cuts };
        n.validate()?;
        Ok(n)
    }

    fn validate(&self) -> Result<(), FuzzyError> {
        let mut prev_lower = f64::NEG_INFINITY;
        let mut prev_upper = f64::INFINITY;
        for ac in &self.cuts {
            if ac.lower < prev_lower - 1e-9 {
                return Err(FuzzyError::LowerNotMonotonic(ac.alpha));
            }
            if ac.upper > prev_upper + 1e-9 {
                return Err(FuzzyError::UpperNotMonotonic(ac.alpha));
            }
            prev_lower = ac.lower;
            prev_upper = ac.upper;
        }
        Ok(())
    }

    /// Garante α=0 e α=1 (extrapolação linear dos cortes adjacentes; senão,
    /// repete o corte mais extremo) — como `canonical()` do Python.
    pub fn canonical(cuts: Vec<AlphaCut>) -> Result<Self, FuzzyError> {
        if cuts.is_empty() {
            return Err(FuzzyError::Empty);
        }
        let mut by_alpha: HashMap<i64, AlphaCut> = HashMap::new();
        for ac in &cuts {
            by_alpha.insert((ac.alpha * 1e12) as i64, *ac);
        }

        let key = |a: f64| (a * 1e12) as i64;
        by_alpha.entry(key(0.0)).or_insert_with(|| {
            let ac0 = cuts
                .iter()
                .min_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap())
                .unwrap();
            let (lower0, upper0) = if cuts.len() >= 2 {
                let mut sorted = cuts.to_vec();
                sorted.sort_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap());
                let ac_next = sorted[1];
                let t = -ac0.alpha / (ac_next.alpha - ac0.alpha);
                (
                    ac0.lower + t * (ac_next.lower - ac0.lower),
                    ac0.upper + t * (ac_next.upper - ac0.upper),
                )
            } else {
                (ac0.lower, ac0.upper)
            };
            AlphaCut {
                alpha: 0.0,
                lower: lower0,
                upper: upper0,
            }
        });

        by_alpha.entry(key(1.0)).or_insert_with(|| {
            let ac1 = cuts
                .iter()
                .max_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap())
                .unwrap();
            let (lower1, upper1) = if cuts.len() >= 2 {
                let mut sorted = cuts.to_vec();
                sorted.sort_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap());
                let ac_prev = sorted[sorted.len() - 2];
                let t = (1.0 - ac_prev.alpha) / (ac1.alpha - ac_prev.alpha);
                (
                    ac_prev.lower + t * (ac1.lower - ac_prev.lower),
                    ac_prev.upper + t * (ac1.upper - ac_prev.upper),
                )
            } else {
                (ac1.lower, ac1.upper)
            };
            AlphaCut {
                alpha: 1.0,
                lower: lower1,
                upper: upper1,
            }
        });

        Self::new(by_alpha.into_values().collect())
    }

    pub fn from_crisp(v: f64) -> Self {
        Self::new(vec![
            AlphaCut {
                alpha: 0.0,
                lower: v,
                upper: v,
            },
            AlphaCut {
                alpha: 1.0,
                lower: v,
                upper: v,
            },
        ])
        .expect("crisp sempre válido")
    }

    pub fn from_interval(lower: f64, upper: f64) -> Self {
        Self::new(vec![
            AlphaCut {
                alpha: 0.0,
                lower,
                upper,
            },
            AlphaCut {
                alpha: 1.0,
                lower,
                upper,
            },
        ])
        .expect("intervalo sempre válido")
    }

    pub fn triangular(a: f64, b: f64, c: f64) -> Result<Self, FuzzyError> {
        if !(a <= b && b <= c) {
            return Err(FuzzyError::BadTriangular);
        }
        let cuts = [0.0, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|&alpha| {
                if alpha == 1.0 {
                    AlphaCut {
                        alpha,
                        lower: b,
                        upper: b,
                    }
                } else {
                    AlphaCut {
                        alpha,
                        lower: a + alpha * (b - a),
                        upper: c - alpha * (c - b),
                    }
                }
            })
            .collect();
        Self::new(cuts)
    }

    fn check_alpha(alpha: f64) -> Result<(), FuzzyError> {
        if !(0.0..=1.0).contains(&alpha) {
            return Err(FuzzyError::AlphaOutOfRange(alpha));
        }
        Ok(())
    }

    fn interp(cuts: &[AlphaCut], alpha: f64, take_lower: bool) -> f64 {
        // np.interp: linear entre cortes adjacentes; fora da faixa, retorna o extremo
        if alpha <= cuts[0].alpha {
            return if take_lower {
                cuts[0].lower
            } else {
                cuts[0].upper
            };
        }
        if alpha >= cuts[cuts.len() - 1].alpha {
            let last = cuts[cuts.len() - 1];
            return if take_lower { last.lower } else { last.upper };
        }
        for w in cuts.windows(2) {
            let (a1, a2) = (w[0], w[1]);
            if alpha >= a1.alpha && alpha <= a2.alpha {
                let t = (alpha - a1.alpha) / (a2.alpha - a1.alpha);
                let (v1, v2) = if take_lower {
                    (a1.lower, a2.lower)
                } else {
                    (a1.upper, a2.upper)
                };
                return v1 + t * (v2 - v1);
            }
        }
        unreachable!()
    }

    pub fn lower_bound(&self, alpha: f64) -> Result<f64, FuzzyError> {
        Self::check_alpha(alpha)?;
        Ok(Self::interp(&self.cuts, alpha, true))
    }

    pub fn upper_bound(&self, alpha: f64) -> Result<f64, FuzzyError> {
        Self::check_alpha(alpha)?;
        Ok(Self::interp(&self.cuts, alpha, false))
    }

    /// Centróide da área sob a pertinência (integração por trapézios entre
    /// cortes consecutivos) — fórmula fatorada contra cancelamento (como o Python).
    pub fn centroid(&self) -> f64 {
        if self.cuts.len() == 1 {
            let ac = self.cuts[0];
            return (ac.lower + ac.upper) / 2.0;
        }
        let mut area = 0.0;
        let mut moment = 0.0;
        for w in self.cuts.windows(2) {
            let (ac1, ac2) = (w[0], w[1]);
            let d_alpha = ac2.alpha - ac1.alpha;
            if d_alpha <= 0.0 {
                continue;
            }
            let avg_lower = (ac1.lower + ac2.lower) / 2.0;
            let avg_upper = (ac1.upper + ac2.upper) / 2.0;
            let width = avg_upper - avg_lower;
            area += width * d_alpha;
            moment += (avg_upper - avg_lower) * (avg_upper + avg_lower) / 2.0 * d_alpha;
        }
        if area.abs() < 1e-12 {
            let ac = self.cuts[0];
            return (ac.lower + ac.upper) / 2.0;
        }
        moment / area
    }

    pub fn support(&self) -> (f64, f64) {
        for ac in &self.cuts {
            if ac.alpha.abs() < 1e-9 {
                return (ac.lower, ac.upper);
            }
        }
        let ac0 = self
            .cuts
            .iter()
            .min_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap())
            .unwrap();
        (ac0.lower, ac0.upper)
    }

    pub fn core(&self) -> (f64, f64) {
        for ac in &self.cuts {
            if (ac.alpha - 1.0).abs() < 1e-9 {
                return (ac.lower, ac.upper);
            }
        }
        let ac = self
            .cuts
            .iter()
            .min_by(|a, b| {
                (a.upper - a.lower)
                    .partial_cmp(&(b.upper - b.lower))
                    .unwrap()
            })
            .unwrap();
        (ac.lower, ac.upper)
    }
}

// ── Princípio de Extensão ──────────────────────────────────────────────────

pub struct ExtensionPrincipleEvaluator<F: Fn(&[f64]) -> f64> {
    score_func: F,
    alphas: Vec<f64>,
}

impl<F: Fn(&[f64]) -> f64> ExtensionPrincipleEvaluator<F> {
    pub fn new(score_func: F, alphas: Vec<f64>) -> Self {
        Self { score_func, alphas }
    }

    /// Para cada α: intervalo exato por enumeração de vértices 2^n (n ≤ 10),
    /// clamp em [0,1], e aninhamento dos cuts (outer contém inner) — fiel ao Python.
    pub fn evaluate(&self, inputs: &[FuzzyNumber]) -> Result<FuzzyNumber, FuzzyError> {
        let n = inputs.len();
        let mut out: Vec<AlphaCut> = Vec::with_capacity(self.alphas.len());

        for &alpha in &self.alphas {
            let lowers: Vec<f64> = inputs
                .iter()
                .map(|i| i.lower_bound(alpha))
                .collect::<Result<_, _>>()?;
            let uppers: Vec<f64> = inputs
                .iter()
                .map(|i| i.upper_bound(alpha))
                .collect::<Result<_, _>>()?;

            let mut min_val = f64::INFINITY;
            let mut max_val = f64::NEG_INFINITY;
            if n <= 10 {
                for combo in 0u32..(1 << n) {
                    let point: Vec<f64> = (0..n)
                        .map(|i| {
                            if (combo >> i) & 1 == 1 {
                                uppers[i]
                            } else {
                                lowers[i]
                            }
                        })
                        .collect();
                    let v = (self.score_func)(&point);
                    min_val = min_val.min(v);
                    max_val = max_val.max(v);
                }
            } else {
                // heurística por gradiente local (como o Python, sem amostragem)
                let centers: Vec<f64> = (0..n).map(|i| (lowers[i] + uppers[i]) / 2.0).collect();
                let base = (self.score_func)(&centers);
                let eps = 1e-6;
                let mut min_point = vec![0.0; n];
                let mut max_point = vec![0.0; n];
                for i in 0..n {
                    let mut perturbed = centers.clone();
                    perturbed[i] += eps;
                    let coeff = ((self.score_func)(&perturbed) - base) / eps;
                    if coeff < 0.0 {
                        min_point[i] = uppers[i];
                        max_point[i] = lowers[i];
                    } else {
                        min_point[i] = lowers[i];
                        max_point[i] = uppers[i];
                    }
                }
                min_val = (self.score_func)(&min_point);
                max_val = (self.score_func)(&max_point);
            }

            min_val = min_val.clamp(0.0, 1.0);
            max_val = max_val.clamp(0.0, 1.0);
            out.push(AlphaCut {
                alpha,
                lower: min_val,
                upper: max_val,
            });
        }

        // Aninhamento: o cut mais largo contém os mais estreitos (como o Python).
        out.sort_by(|a, b| a.alpha.partial_cmp(&b.alpha).unwrap());
        for i in (0..out.len().saturating_sub(1)).rev() {
            let inner = out[i + 1];
            let outer = out[i];
            out[i] = AlphaCut {
                alpha: outer.alpha,
                lower: outer.lower.min(inner.lower),
                upper: outer.upper.max(inner.upper),
            };
        }

        FuzzyNumber::new(out)
    }
}

// ── Seletor (perfis canônicos do Python) ───────────────────────────────────

/// Score de um perfil com intervalos fuzzy e bounds α=0.8.
#[derive(Debug, Clone)]
pub struct ProfileScore {
    pub profile: QoSProfile,
    pub fuzzy_score: FuzzyNumber,
    pub centroid: f64,
    pub lower_08: f64,
    pub upper_08: f64,
}

/// Seletor Zadeh com os pesos canônicos de `qos_selector.py` (incl. Balanced
/// corrigido: error_rate −0.20, historical_confidence +0.20, demais −0.15).
pub struct ZadehSelector {
    profiles: Vec<(QoSProfile, Vec<(&'static str, f64)>)>,
    alphas: Vec<f64>,
}

impl Default for ZadehSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl ZadehSelector {
    pub fn new() -> Self {
        let profiles = vec![
            (
                QoSProfile::Critical,
                vec![
                    ("urgency", 0.30),
                    ("deadline_pressure", 0.20),
                    ("recent_latency", -0.15),
                    ("recent_ttft", -0.10),
                    ("historical_confidence", 0.10),
                    ("agent_load", -0.10),
                    ("estimated_complexity", 0.05),
                ],
            ),
            (
                QoSProfile::Failover,
                vec![
                    ("error_rate", 0.25),
                    ("recent_latency", 0.20),
                    ("agent_load", 0.15),
                    ("historical_confidence", -0.15),
                    ("deadline_pressure", 0.15),
                    ("urgency", 0.10),
                ],
            ),
            (
                QoSProfile::StreamLike,
                vec![
                    ("streaming_need", 0.35),
                    ("urgency", 0.20),
                    ("recent_ttft", -0.15),
                    ("recent_latency", -0.10),
                    ("agent_load", -0.10),
                    ("historical_confidence", 0.10),
                ],
            ),
            (
                QoSProfile::LowCost,
                vec![
                    ("urgency", -0.30),
                    ("estimated_complexity", -0.25),
                    ("allowed_cost", -0.20),
                    ("streaming_need", -0.15),
                    ("deadline_pressure", -0.10),
                ],
            ),
            (
                QoSProfile::Balanced,
                vec![
                    ("error_rate", -0.20),
                    ("historical_confidence", 0.20),
                    ("agent_load", -0.15),
                    ("recent_latency", -0.15),
                    ("recent_ttft", -0.15),
                    ("urgency", -0.15),
                ],
            ),
        ];
        Self {
            profiles,
            alphas: vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0],
        }
    }

    fn score_of(
        &self,
        weights: &[(&str, f64)],
        inputs: &HashMap<&'static str, FuzzyNumber>,
    ) -> ProfileScore {
        let keys: Vec<&str> = weights.iter().map(|(k, _)| *k).collect();
        let wmap: HashMap<&str, f64> = weights.iter().cloned().collect();
        // Peso negativo ⇒ |w|·(1−val); positivo ⇒ w·val (como _make_score_func do Python)
        let score_func = move |point: &[f64]| -> f64 {
            keys.iter()
                .enumerate()
                .map(|(i, k)| {
                    let val = point.get(i).copied().unwrap_or(0.5);
                    let w = wmap[k];
                    if w < 0.0 {
                        w.abs() * (1.0 - val)
                    } else {
                        w * val
                    }
                })
                .sum()
        };

        let input_list: Vec<FuzzyNumber> = weights
            .iter()
            .map(|(k, _)| {
                inputs
                    .get(*k)
                    .cloned()
                    .unwrap_or_else(|| FuzzyNumber::from_crisp(0.5)) // default Python (warn)
            })
            .collect();

        let evaluator = ExtensionPrincipleEvaluator::new(score_func, self.alphas.clone());
        let fuzzy_score = evaluator.evaluate(&input_list).expect("evaluate");
        let centroid = fuzzy_score.centroid();
        let lower_08 = fuzzy_score.lower_bound(0.8).expect("lower 0.8");
        let upper_08 = fuzzy_score.upper_bound(0.8).expect("upper 0.8");

        ProfileScore {
            profile: QoSProfile::Balanced, // sobrescrito pelo caller
            fuzzy_score,
            centroid,
            lower_08,
            upper_08,
        }
    }

    /// Avalia todos os perfis (como `evaluate_all`).
    pub fn evaluate_all(&self, inputs: &HashMap<&'static str, FuzzyNumber>) -> Vec<ProfileScore> {
        self.profiles
            .iter()
            .map(|(profile, weights)| {
                let mut s = self.score_of(weights, inputs);
                s.profile = profile.clone();
                s
            })
            .collect()
    }

    /// Seleciona o melhor perfil. `conservative=true` (default do Python):
    /// critério (lower_0.8, centroid); senão (centroid, lower_0.8).
    pub fn select(
        &self,
        inputs: &HashMap<&'static str, FuzzyNumber>,
        conservative: bool,
    ) -> ProfileScore {
        let scores = self.evaluate_all(inputs);
        let cmp = |a: &ProfileScore, b: &ProfileScore| {
            let (ka, kb) = if conservative {
                ((a.lower_08, a.centroid), (b.lower_08, b.centroid))
            } else {
                ((a.centroid, a.lower_08), (b.centroid, b.lower_08))
            };
            ka.partial_cmp(&kb).unwrap()
        };
        scores
            .into_iter()
            .max_by(|a, b| cmp(a, b))
            .expect("profiles não vazio")
    }
}

// ── Fachada QosDecider (métricas crisp → fuzzy) ────────────────────────────

/// `QosDecider` Zadeh: converte métricas crisp em `FuzzyNumber::from_crisp` e
/// usa o seletor por extensão com seleção conservadora (como o Python).
pub struct ZadehDecider {
    selector: ZadehSelector,
}

impl ZadehDecider {
    pub fn new() -> Self {
        Self {
            selector: ZadehSelector::new(),
        }
    }

    pub fn selector(&self) -> &ZadehSelector {
        &self.selector
    }
}

impl Default for ZadehDecider {
    fn default() -> Self {
        Self::new()
    }
}

impl QosDecider for ZadehDecider {
    fn decide(&self, metrics: &QoSMetrics) -> QoSDecision {
        let inputs: HashMap<&'static str, FuzzyNumber> = [
            ("urgency", FuzzyNumber::from_crisp(metrics.urgency)),
            (
                "deadline_pressure",
                FuzzyNumber::from_crisp(metrics.deadline_pressure),
            ),
            (
                "recent_latency",
                FuzzyNumber::from_crisp(metrics.recent_latency),
            ),
            ("agent_load", FuzzyNumber::from_crisp(metrics.agent_load)),
            ("error_rate", FuzzyNumber::from_crisp(metrics.error_rate)),
            (
                "historical_confidence",
                FuzzyNumber::from_crisp(metrics.historical_confidence),
            ),
            (
                "estimated_complexity",
                FuzzyNumber::from_crisp(metrics.estimated_complexity),
            ),
            (
                "streaming_need",
                FuzzyNumber::from_crisp(metrics.streaming_need),
            ),
        ]
        .into_iter()
        .collect();

        let best = self.selector.select(&inputs, true);
        QoSDecision {
            profile: best.profile,
            confidence: best.centroid,
            explanation: format!(
                "zadeh(ext): centroid={:.3}, α0.8=[{:.3},{:.3}]",
                best.centroid, best.lower_08, best.upper_08
            ),
        }
    }

    fn name(&self) -> &str {
        "zadeh"
    }
}
