//! Baselines comparadores do artigo — porte fiel de
//! `src/orchestrator/benchmarks/experiments/baselines/` (Python).
//!
//! - [`FixedRulesDecider`] ← `fixed_rules_baseline.py` (thresholds nítidos,
//!   prática comum dos orquestradores DDS atuais)
//! - [`MamdaniDecider`] ← `mamdani_baseline.py` (27 regras skfuzzy, AND=min,
//!   agregação=max, defuzzificação por centróide no universo discreto
//!   `np.arange(1, 5.1, 0.1)`)
//! - [`Ucb1Decider`] ← `ucb1_baseline.py` (bandit estacionário; estado com
//!   interior mutability via `Mutex`, mesmo padrão do `FcmDhlDecider`)
//! - [`SwUcbDecider`] ← `sw_ucb_baseline.py` (janela deslizante, Garivier &
//!   Moulines 2011; `Mutex`)
//!
//! ## Nota sobre os cenários canônicos (`dataset.rs::CANONICAL`)
//!
//! Os rótulos canônicos (ocioso `→ Balanced`, degradado `→ Failover`) são a
//! referência do **NFCM**. Os baselines fiéis ao Python **não** reproduzem
//! esses rótulos nesses dois pontos — e é exatamente essa lacuna que o
//! artigo quantifica:
//!
//! | cenário (métricas) | NFCM (ref.) | FixedRules | Mamdani |
//! |---|---|---|---|
//! | ocioso `[0.10,0.05,0.15,0.10,0.05,0.90,0.20,0.05]` | Balanced | LowCost (`urgency<0.2 && load<0.3`) | Critical (low,low,low) |
//! | degradado `[0.60,0.40,0.85,0.80,0.90,0.20,0.50,0.10]` | Failover | Failover (`error_rate>0.3`) | LowCost (high,high,high) |
//!
//! Os testes deste módulo assertam o comportamento **fiel ao Python**, com os
//! rótulos NFCM indicados em comentário como referência.

use crate::decider::{QoSDecision, QoSMetrics, QosDecider};
use crate::QoSProfile;
use std::collections::VecDeque;
use std::sync::Mutex;

// ── Perfis como braços (ordem do PROFILES dos baselines Python) ─────────────

/// Ordem dos braços igual a `PROFILES` de `ucb1_baseline.py:25` /
/// `sw_ucb_baseline.py:28`.
pub const ARM_PROFILES: [QoSProfile; 5] = [
    QoSProfile::Critical,
    QoSProfile::Failover,
    QoSProfile::StreamLike,
    QoSProfile::LowCost,
    QoSProfile::Balanced,
];

fn arm_index(profile: &QoSProfile) -> Option<usize> {
    ARM_PROFILES.iter().position(|p| p == profile)
}

fn profile_name(profile: &QoSProfile) -> &'static str {
    match profile {
        QoSProfile::Critical => "QoS_Critical",
        QoSProfile::Failover => "QoS_Failover",
        QoSProfile::StreamLike => "QoS_StreamLike",
        QoSProfile::LowCost => "QoS_LowCost",
        QoSProfile::Balanced => "QoS_Balanced",
    }
}

// ── FixedRules (fixed_rules_baseline.py) ────────────────────────────────────

/// Baseline determinístico de regras fixas (thresholds nítidos, sem
/// fuzzificação) — porte 1:1 de `FixedRulesBaseline.select`
/// (`fixed_rules_baseline.py:19-48`).
pub struct FixedRulesDecider;

impl FixedRulesDecider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FixedRulesDecider {
    fn default() -> Self {
        Self::new()
    }
}

impl QosDecider for FixedRulesDecider {
    fn decide(&self, metrics: &QoSMetrics) -> QoSDecision {
        let urgency = metrics.urgency;
        let latency = metrics.recent_latency;
        let error_rate = metrics.error_rate;
        let cpu_load = metrics.agent_load;

        // Mesma cadeia if/elif do Python (ordem importa).
        let (profile, rule) = if urgency > 0.8 && latency < 0.3 {
            (QoSProfile::Critical, "urgency>0.8 && latency<0.3")
        } else if error_rate > 0.3 || (latency > 0.7 && cpu_load > 0.7) {
            (
                QoSProfile::Failover,
                "error_rate>0.3 || (latency>0.7 && load>0.7)",
            )
        } else if urgency < 0.2 && cpu_load < 0.3 {
            (QoSProfile::LowCost, "urgency<0.2 && load<0.3")
        } else if metrics.streaming_need > 0.7 {
            (QoSProfile::StreamLike, "streaming_need>0.7")
        } else {
            (QoSProfile::Balanced, "else")
        };

        QoSDecision {
            confidence: 1.0,
            explanation: format!("fixed-rules: {} [{}]", profile_name(&profile), rule),
            profile,
        }
    }

    fn name(&self) -> &str {
        "fixed-rules"
    }
}

// ── Mamdani (mamdani_baseline.py) ───────────────────────────────────────────

/// Função de pertinência triangular (`fuzz.trimf` do scikit-fuzzy).
fn trimf(x: f64, a: f64, b: f64, c: f64) -> f64 {
    if x < a || x > c {
        return 0.0;
    }
    if x == b {
        // cobre a==b (low/critical) e b==c (high/lowcost): μ(b)=1
        return 1.0;
    }
    if x < b {
        (x - a) / (b - a)
    } else {
        (c - x) / (c - b)
    }
}

/// Níveis linguísticos das entradas (índices 0/1/2).
const LOW: usize = 0;
const MEDIUM: usize = 1;
const HIGH: usize = 2;

/// Consequentes (índice no vetor de MFs de saída).
const OUT_CRITICAL: usize = 0;
const OUT_FAILOVER: usize = 1;
const OUT_STREAMLIKE: usize = 2;
const OUT_BALANCED: usize = 3;
const OUT_LOWCOST: usize = 4;

/// MFs de entrada — `mamdani_baseline.py:51-53`:
/// low `[0,0,0.4]`, medium `[0.2,0.5,0.8]`, high `[0.6,1,1]`.
const INPUT_MFS: [(f64, f64, f64); 3] = [(0.0, 0.0, 0.4), (0.2, 0.5, 0.8), (0.6, 1.0, 1.0)];

/// MFs de saída — `mamdani_baseline.py:56-60`.
const OUTPUT_MFS: [(f64, f64, f64); 5] = [
    (1.0, 1.0, 1.5), // critical
    (1.5, 2.0, 2.5), // failover
    (2.5, 3.0, 3.5), // streamlike
    (3.5, 4.0, 4.5), // balanced
    (4.5, 5.0, 5.0), // lowcost
];

/// As 27 regras — `mamdani_baseline.py:65-92` (duplicata da regra 6 na linha 89
/// preservada: idempotente na agregação por max).
#[allow(clippy::type_complexity)]
const RULES: [(usize, usize, usize, usize); 27] = [
    (LOW, LOW, LOW, OUT_CRITICAL),          // l.65
    (LOW, LOW, MEDIUM, OUT_CRITICAL),       // l.66
    (LOW, MEDIUM, LOW, OUT_FAILOVER),       // l.67
    (HIGH, HIGH, HIGH, OUT_LOWCOST),        // l.69
    (HIGH, LOW, LOW, OUT_BALANCED),         // l.70
    (MEDIUM, MEDIUM, MEDIUM, OUT_BALANCED), // l.71
    (LOW, HIGH, LOW, OUT_FAILOVER),         // l.72
    (LOW, HIGH, HIGH, OUT_FAILOVER),        // l.73
    (MEDIUM, LOW, LOW, OUT_STREAMLIKE),     // l.74
    (MEDIUM, LOW, MEDIUM, OUT_STREAMLIKE),  // l.75
    (MEDIUM, HIGH, LOW, OUT_FAILOVER),      // l.76
    (MEDIUM, HIGH, HIGH, OUT_LOWCOST),      // l.77
    (HIGH, LOW, MEDIUM, OUT_BALANCED),      // l.78
    (HIGH, LOW, HIGH, OUT_LOWCOST),         // l.79
    (HIGH, MEDIUM, LOW, OUT_BALANCED),      // l.80
    (HIGH, MEDIUM, MEDIUM, OUT_LOWCOST),    // l.81
    (HIGH, MEDIUM, HIGH, OUT_LOWCOST),      // l.82
    (LOW, MEDIUM, MEDIUM, OUT_FAILOVER),    // l.83
    (LOW, MEDIUM, HIGH, OUT_FAILOVER),      // l.84
    (MEDIUM, MEDIUM, LOW, OUT_STREAMLIKE),  // l.85
    (HIGH, HIGH, LOW, OUT_LOWCOST),         // l.86
    (HIGH, HIGH, MEDIUM, OUT_LOWCOST),      // l.87
    (LOW, LOW, HIGH, OUT_CRITICAL),         // l.88
    (MEDIUM, MEDIUM, MEDIUM, OUT_BALANCED), // l.89 (dup da l.71)
    (MEDIUM, HIGH, MEDIUM, OUT_FAILOVER),   // l.90
    (LOW, HIGH, MEDIUM, OUT_FAILOVER),      // l.91
    (MEDIUM, LOW, HIGH, OUT_BALANCED),      // l.92
];

/// Índice de perfil defuzzificado — `None` quando a soma das pertinências é
/// zero (no Python, `compute()` estoura e `select` cai no `_fallback_select`,
/// `mamdani_baseline.py:119-120`).
fn mamdani_index(latency: f64, packet_loss: f64, cpu: f64) -> Option<f64> {
    let fuzzify = |x: f64| -> [f64; 3] {
        [
            trimf(x, INPUT_MFS[LOW].0, INPUT_MFS[LOW].1, INPUT_MFS[LOW].2),
            trimf(
                x,
                INPUT_MFS[MEDIUM].0,
                INPUT_MFS[MEDIUM].1,
                INPUT_MFS[MEDIUM].2,
            ),
            trimf(x, INPUT_MFS[HIGH].0, INPUT_MFS[HIGH].1, INPUT_MFS[HIGH].2),
        ]
    };
    let mu_l = fuzzify(latency);
    let mu_p = fuzzify(packet_loss);
    let mu_c = fuzzify(cpu);

    // Ativação por consequente: AND=min nos antecedentes, agregação=max.
    let mut act = [0.0f64; 5];
    for (l, p, c, out) in RULES {
        let a = mu_l[l].min(mu_p[p]).min(mu_c[c]);
        if a > act[out] {
            act[out] = a;
        }
    }

    // Centróide no universo discreto np.arange(1, 5.1, 0.1) — 41 pontos,
    // implicação por corte (min), como o skfuzzy.
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..41 {
        let x = 1.0 + f64::from(i) * 0.1;
        let mut m = 0.0f64;
        for (k, (a, b, c)) in OUTPUT_MFS.iter().enumerate() {
            m = m.max(act[k].min(trimf(x, *a, *b, *c)));
        }
        num += x * m;
        den += m;
    }
    if den <= 0.0 {
        return None;
    }
    Some(num / den)
}

/// Baseline Mamdani — porte de `MamdaniBaseline` (`mamdani_baseline.py`).
///
/// Entradas (clamp [0,1], linhas 112-114): `recent_latency` → latency,
/// `error_rate` → packet_loss, `agent_load` → cpu.
pub struct MamdaniDecider;

impl MamdaniDecider {
    pub fn new() -> Self {
        Self
    }

    /// Índice numérico defuzzificado (expõe para testes/observabilidade).
    pub fn defuzzified_index(&self, metrics: &QoSMetrics) -> Option<f64> {
        let clamp01 = |v: f64| v.clamp(0.0, 1.0);
        mamdani_index(
            clamp01(metrics.recent_latency),
            clamp01(metrics.error_rate),
            clamp01(metrics.agent_load),
        )
    }
}

impl Default for MamdaniDecider {
    fn default() -> Self {
        Self::new()
    }
}

impl QosDecider for MamdaniDecider {
    fn decide(&self, metrics: &QoSMetrics) -> QoSDecision {
        match self.defuzzified_index(metrics) {
            Some(idx) => {
                // `_idx_to_profile` (mamdani_baseline.py:123-134)
                let profile = if idx < 1.5 {
                    QoSProfile::Critical
                } else if idx < 2.5 {
                    QoSProfile::Failover
                } else if idx < 3.5 {
                    QoSProfile::StreamLike
                } else if idx < 4.5 {
                    QoSProfile::Balanced
                } else {
                    QoSProfile::LowCost
                };
                QoSDecision {
                    confidence: 1.0,
                    explanation: format!(
                        "mamdani: {} (profile_idx={:.3})",
                        profile_name(&profile),
                        idx
                    ),
                    profile,
                }
            }
            None => {
                // `_fallback_select` (mamdani_baseline.py:137-144)
                let profile = if metrics.urgency > 0.8 {
                    QoSProfile::Critical
                } else if metrics.urgency < 0.2 {
                    QoSProfile::LowCost
                } else {
                    QoSProfile::Balanced
                };
                QoSDecision {
                    confidence: 1.0,
                    explanation: format!(
                        "mamdani-fallback: {} (urgency={:.2})",
                        profile_name(&profile),
                        metrics.urgency
                    ),
                    profile,
                }
            }
        }
    }

    fn name(&self) -> &str {
        "mamdani"
    }
}

// ── UCB1 (ucb1_baseline.py) ─────────────────────────────────────────────────

struct Ucb1Inner {
    counts: [u64; 5],
    values: [f64; 5],
    total: u64,
}

/// Baseline UCB1 (bandit estacionário) — porte de `UCB1Baseline`
/// (`ucb1_baseline.py`). Ignora as métricas (como o Python); a recompensa
/// esperada é a negativa da latência E2E normalizada.
///
/// Estado interno via `Mutex` (interior mutability — padrão do
/// `FcmDhlDecider`), pois `QosDecider::decide` recebe `&self`.
pub struct Ucb1Decider {
    inner: Mutex<Ucb1Inner>,
}

impl Ucb1Decider {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Ucb1Inner {
                counts: [0; 5],
                values: [0.0; 5],
                total: 0,
            }),
        }
    }

    /// Atualiza estatísticas do braço após observar a recompensa
    /// (`ucb1_baseline.py:58-74` — média incremental).
    pub fn update(&self, profile: &QoSProfile, reward: f64) {
        let Some(arm) = arm_index(profile) else {
            return;
        };
        let mut inner = self.inner.lock().unwrap();
        inner.counts[arm] += 1;
        inner.total += 1;
        let n = inner.counts[arm] as f64;
        inner.values[arm] = inner.values[arm] * (n - 1.0) / n + reward / n;
    }

    /// Reseta estatísticas (`ucb1_baseline.py:76-80`).
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.counts = [0; 5];
        inner.values = [0.0; 5];
        inner.total = 0;
    }

    /// Contagens por braço (observabilidade/testes).
    pub fn counts(&self) -> [u64; 5] {
        self.inner.lock().unwrap().counts
    }
}

impl Default for Ucb1Decider {
    fn default() -> Self {
        Self::new()
    }
}

impl QosDecider for Ucb1Decider {
    fn decide(&self, _metrics: &QoSMetrics) -> QoSDecision {
        let inner = self.inner.lock().unwrap();

        // Primeira rodada: explorar cada braço uma vez (ordem de PROFILES).
        for (i, &c) in inner.counts.iter().enumerate() {
            if c == 0 {
                return QoSDecision {
                    profile: ARM_PROFILES[i].clone(),
                    confidence: 1.0,
                    explanation: format!(
                        "ucb1: explore arm {} (counts=0)",
                        profile_name(&ARM_PROFILES[i])
                    ),
                };
            }
        }

        // UCB1: value + sqrt(2·ln(total)/count) — argmax com desempate no
        // primeiro índice (como np.argmax).
        let ln_total = (inner.total as f64).ln();
        let mut best = 0usize;
        let mut best_ucb = f64::NEG_INFINITY;
        for i in 0..5 {
            let ucb = inner.values[i] + (2.0 * ln_total / inner.counts[i] as f64).sqrt();
            if ucb > best_ucb {
                best_ucb = ucb;
                best = i;
            }
        }

        QoSDecision {
            profile: ARM_PROFILES[best].clone(),
            confidence: best_ucb,
            explanation: format!(
                "ucb1: {} (ucb={:.3}, n={}, total={})",
                profile_name(&ARM_PROFILES[best]),
                best_ucb,
                inner.counts[best],
                inner.total
            ),
        }
    }

    fn name(&self) -> &str {
        "ucb1"
    }
}

// ── SW-UCB (sw_ucb_baseline.py) ─────────────────────────────────────────────

struct SwUcbInner {
    windows: [VecDeque<f64>; 5],
    window_size: usize,
    total: u64,
}

/// Baseline Sliding-Window UCB (ambientes não-estacionários) — porte de
/// `SWUCBBaseline` (`sw_ucb_baseline.py`; Garivier & Moulines, ALT 2011).
pub struct SwUcbDecider {
    inner: Mutex<SwUcbInner>,
}

impl SwUcbDecider {
    /// `window_size` default 100 (`sw_ucb_baseline.py:30`).
    pub fn new(window_size: usize) -> Self {
        Self {
            inner: Mutex::new(SwUcbInner {
                windows: std::array::from_fn(|_| VecDeque::with_capacity(window_size)),
                window_size,
                total: 0,
            }),
        }
    }

    /// Registra recompensa do braço (`sw_ucb_baseline.py:67-80` — deque
    /// com maxlen: descarta a mais antiga quando cheio).
    pub fn update(&self, profile: &QoSProfile, reward: f64) {
        let Some(arm) = arm_index(profile) else {
            return;
        };
        let mut inner = self.inner.lock().unwrap();
        let window_size = inner.window_size;
        let w = &mut inner.windows[arm];
        if w.len() >= window_size {
            w.pop_front();
        }
        w.push_back(reward);
        inner.total += 1;
    }

    /// Reseta estatísticas (`sw_ucb_baseline.py:82-85`).
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        for w in &mut inner.windows {
            w.clear();
        }
        inner.total = 0;
    }

    /// Tamanhos das janelas por braço (observabilidade/testes).
    pub fn window_lens(&self) -> [usize; 5] {
        let inner = self.inner.lock().unwrap();
        std::array::from_fn(|i| inner.windows[i].len())
    }
}

impl Default for SwUcbDecider {
    fn default() -> Self {
        Self::new(100)
    }
}

impl QosDecider for SwUcbDecider {
    fn decide(&self, _metrics: &QoSMetrics) -> QoSDecision {
        let inner = self.inner.lock().unwrap();

        // Explorar cada braço uma vez (janela vazia), na ordem de PROFILES.
        for (i, w) in inner.windows.iter().enumerate() {
            if w.is_empty() {
                return QoSDecision {
                    profile: ARM_PROFILES[i].clone(),
                    confidence: 1.0,
                    explanation: format!(
                        "sw-ucb: explore arm {} (janela vazia)",
                        profile_name(&ARM_PROFILES[i])
                    ),
                };
            }
        }

        // SW-UCB: mean_w + sqrt(2·ln(min(total, W))/n_i) — argmax com
        // desempate no primeiro índice (np.argmax).
        let ln = (inner.total.min(inner.window_size as u64) as f64).ln();
        let mut best = 0usize;
        let mut best_ucb = f64::NEG_INFINITY;
        for (i, w) in inner.windows.iter().enumerate() {
            let n_i = w.len() as f64;
            let mean_i: f64 = w.iter().sum::<f64>() / n_i;
            let ucb = mean_i + (2.0 * ln / n_i).sqrt();
            if ucb > best_ucb {
                best_ucb = ucb;
                best = i;
            }
        }

        QoSDecision {
            profile: ARM_PROFILES[best].clone(),
            confidence: best_ucb,
            explanation: format!(
                "sw-ucb: {} (ucb={:.3}, n_w={}, total={})",
                profile_name(&ARM_PROFILES[best]),
                best_ucb,
                inner.windows[best].len(),
                inner.total
            ),
        }
    }

    fn name(&self) -> &str {
        "sw-ucb"
    }
}

// ── Testes ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Cenário canônico "ocioso" (`dataset.rs::CANONICAL[0]`) — ref. NFCM: Balanced.
    const OC_IOSO: [f64; 8] = [0.10, 0.05, 0.15, 0.10, 0.05, 0.90, 0.20, 0.05];
    /// Cenário canônico "degradado" (`CANONICAL[2]`) — ref. NFCM: Failover.
    const DEGRADADO: [f64; 8] = [0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10];

    fn metrics(x: [f64; 8]) -> QoSMetrics {
        QoSMetrics {
            urgency: x[0],
            deadline_pressure: x[1],
            recent_latency: x[2],
            agent_load: x[3],
            error_rate: x[4],
            historical_confidence: x[5],
            estimated_complexity: x[6],
            streaming_need: x[7],
        }
    }

    // ── FixedRules ───────────────────────────────────────────────────────

    #[test]
    fn fixed_rules_cobre_os_cinco_ramos() {
        let d = FixedRulesDecider::new();
        let m = |u, l, e, c, s| QoSMetrics {
            urgency: u,
            recent_latency: l,
            error_rate: e,
            agent_load: c,
            streaming_need: s,
            ..Default::default()
        };
        assert_eq!(
            d.decide(&m(0.9, 0.2, 0.05, 0.5, 0.0)).profile,
            QoSProfile::Critical
        );
        assert_eq!(
            d.decide(&m(0.5, 0.4, 0.5, 0.5, 0.0)).profile,
            QoSProfile::Failover
        );
        assert_eq!(
            d.decide(&m(0.5, 0.8, 0.1, 0.8, 0.0)).profile,
            QoSProfile::Failover
        );
        assert_eq!(
            d.decide(&m(0.1, 0.4, 0.05, 0.2, 0.0)).profile,
            QoSProfile::LowCost
        );
        assert_eq!(
            d.decide(&m(0.5, 0.4, 0.05, 0.5, 0.9)).profile,
            QoSProfile::StreamLike
        );
        assert_eq!(
            d.decide(&m(0.5, 0.4, 0.05, 0.5, 0.3)).profile,
            QoSProfile::Balanced
        );
        assert_eq!(d.name(), "fixed-rules");
    }

    /// Discriminação dos cenários canônicos pelo baseline FixedRules.
    /// Ref. NFCM: ocioso→Balanced, degradado→Failover. O baseline acerta o
    /// degradado mas classifica o ocioso como LowCost (urgency<0.2 && load<0.3)
    /// — comportamento fiel a `fixed_rules_baseline.py:43-44`.
    #[test]
    fn fixed_rules_nos_cenarios_canonicos() {
        let d = FixedRulesDecider::new();
        assert_eq!(d.decide(&metrics(OC_IOSO)).profile, QoSProfile::LowCost);
        assert_eq!(d.decide(&metrics(DEGRADADO)).profile, QoSProfile::Failover);
    }

    // ── Mamdani ──────────────────────────────────────────────────────────

    #[test]
    fn mamdani_todo_medio_da_balanced() {
        let d = MamdaniDecider::new();
        let m = QoSMetrics {
            recent_latency: 0.5,
            error_rate: 0.5,
            agent_load: 0.5,
            ..Default::default()
        };
        let idx = d.defuzzified_index(&m).unwrap();
        assert!((idx - 4.0).abs() < 0.15, "idx={idx}");
        assert_eq!(d.decide(&m).profile, QoSProfile::Balanced);
        assert_eq!(d.name(), "mamdani");
    }

    /// Discriminação dos cenários canônicos pelo baseline Mamdani.
    /// Ref. NFCM: ocioso→Balanced, degradado→Failover. Fiel ao skfuzzy
    /// (27 regras), o ocioso (low,low,low) vira Critical e o degradado
    /// (high,high,high) vira LowCost — ver tabela no topo do módulo.
    #[test]
    fn mamdani_nos_cenarios_canonicos() {
        let d = MamdaniDecider::new();
        let idx_ocioso = d.defuzzified_index(&metrics(OC_IOSO)).unwrap();
        assert!(idx_ocioso < 1.5, "idx_ocioso={idx_ocioso}");
        assert_eq!(d.decide(&metrics(OC_IOSO)).profile, QoSProfile::Critical);

        let idx_degradado = d.defuzzified_index(&metrics(DEGRADADO)).unwrap();
        assert!(idx_degradado >= 4.5, "idx_degradado={idx_degradado}");
        assert_eq!(d.decide(&metrics(DEGRADADO)).profile, QoSProfile::LowCost);
    }

    #[test]
    fn trimf_nos_vertices_e_meio() {
        assert_eq!(trimf(0.0, 0.0, 0.0, 0.4), 1.0);
        assert!((trimf(0.15, 0.0, 0.0, 0.4) - 0.625).abs() < 1e-12);
        assert_eq!(trimf(0.5, 0.2, 0.5, 0.8), 1.0);
        assert!((trimf(0.85, 0.6, 1.0, 1.0) - 0.625).abs() < 1e-12);
        assert_eq!(trimf(5.0, 4.5, 5.0, 5.0), 1.0);
        assert_eq!(trimf(0.9, 0.2, 0.5, 0.8), 0.0);
    }

    // ── UCB1 ─────────────────────────────────────────────────────────────

    #[test]
    fn ucb1_explora_bracos_na_ordem() {
        let d = Ucb1Decider::new();
        let m = QoSMetrics::default();
        for esperado in [
            QoSProfile::Critical,
            QoSProfile::Failover,
            QoSProfile::StreamLike,
            QoSProfile::LowCost,
            QoSProfile::Balanced,
        ] {
            assert_eq!(d.decide(&m).profile, esperado);
            d.update(&esperado, 0.5);
        }
        assert_eq!(d.counts(), [1, 1, 1, 1, 1]);
        assert_eq!(d.name(), "ucb1");
    }

    #[test]
    fn ucb1_explora_depois_explora_melhor_braco() {
        let d = Ucb1Decider::new();
        let m = QoSMetrics::default();
        // Todos os braços com recompensa baixa, Balanced com alta.
        for p in ARM_PROFILES.iter() {
            let r = if *p == QoSProfile::Balanced { 0.9 } else { 0.1 };
            d.update(p, r);
        }
        // Amostras adicionais derrubam o bônus de exploração dos braços ruins.
        for _ in 0..20 {
            d.update(&QoSProfile::Balanced, 0.9);
            d.update(&QoSProfile::Critical, 0.1);
            d.update(&QoSProfile::Failover, 0.1);
            d.update(&QoSProfile::StreamLike, 0.1);
            d.update(&QoSProfile::LowCost, 0.1);
        }
        assert_eq!(d.decide(&m).profile, QoSProfile::Balanced);

        d.reset();
        assert_eq!(d.counts(), [0; 5]);
        assert_eq!(d.decide(&m).profile, QoSProfile::Critical);
    }

    // ── SW-UCB ───────────────────────────────────────────────────────────

    #[test]
    fn sw_ucb_explora_bracos_na_ordem() {
        let d = SwUcbDecider::new(10);
        let m = QoSMetrics::default();
        for esperado in [
            QoSProfile::Critical,
            QoSProfile::Failover,
            QoSProfile::StreamLike,
            QoSProfile::LowCost,
            QoSProfile::Balanced,
        ] {
            assert_eq!(d.decide(&m).profile, esperado);
            d.update(&esperado, 0.5);
        }
        assert_eq!(d.window_lens(), [1, 1, 1, 1, 1]);
        assert_eq!(d.name(), "sw-ucb");
    }

    #[test]
    fn sw_ucb_esquece_regime_antigo_pela_janela() {
        let d = SwUcbDecider::new(4);
        let m = QoSMetrics::default();
        // Regime 1: Critical é o melhor.
        for p in ARM_PROFILES.iter() {
            let r = if *p == QoSProfile::Critical { 1.0 } else { 0.0 };
            for _ in 0..4 {
                d.update(p, r);
            }
        }
        assert_eq!(d.decide(&m).profile, QoSProfile::Critical);

        // Regime 2 (não-estacionário): Critical piora, Balanced melhora.
        // A janela (W=4) esquece o regime 1.
        for _ in 0..4 {
            d.update(&QoSProfile::Critical, 0.0);
        }
        for _ in 0..4 {
            d.update(&QoSProfile::Balanced, 1.0);
        }
        assert_eq!(d.decide(&m).profile, QoSProfile::Balanced);

        d.reset();
        assert_eq!(d.window_lens(), [0; 5]);
        assert_eq!(d.decide(&m).profile, QoSProfile::Critical);
    }

    #[test]
    fn sw_ucb_janela_respeita_maxlen() {
        let d = SwUcbDecider::new(3);
        for _ in 0..10 {
            d.update(&QoSProfile::Critical, 1.0);
        }
        assert_eq!(d.window_lens()[0], 3);
    }
}
