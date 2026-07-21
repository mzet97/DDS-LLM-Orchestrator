//! FCM (Kosko) com clamp das entradas + detecção de atrator + DHL (REQ-503).
//!
//! Porte fiel de `fcm_qos_manager/` (Python):
//! - `fcm.py` → [`FuzzyCognitiveMap`] (dinâmica sigmoide com conceitos de
//!   entrada **clampados** no valor medido; parada em ponto fixo/ciclo-limite)
//! - `profile.py` → [`build_weight_matrix`]/[`build_qos_fcm`] (arestas entrada→
//!   decisão do QoSSelector + arestas ENTRE conceitos, o que distingue FCM de linear)
//! - `dhl.py` → [`DifferentialHebbianLearner`] (Δw = c_t·(ΔC_i·ΔC_j − w), c_t decai)

use crate::decider::{QoSDecision, QoSMetrics, QosDecider};
use crate::QoSProfile;
use std::collections::{HashMap, HashSet};

// ── Erros ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum FcmError {
    #[error("conceito duplicado")]
    DuplicateConcept,
    #[error("aresta ({0}->{1}) referencia conceito inexistente")]
    UnknownConcept(String, String),
    #[error("peso fora de [-1,1] em ({0}->{1}): {2}")]
    WeightOutOfRange(String, String, f64),
}

// ── Sigmoide (clamp anti-overflow, como o Python) ─────────────────────────

pub fn sigmoid(x: f64, lam: f64) -> f64 {
    let z = -lam * x;
    if z > 60.0 {
        return 0.0;
    }
    if z < -60.0 {
        return 1.0;
    }
    1.0 / (1.0 + z.exp())
}

// ── Motor FCM ──────────────────────────────────────────────────────────────

type Edge = (String, String);

/// FCM com dinâmica sigmoide e detecção de atrator (porte de `fcm.py`).
pub struct FuzzyCognitiveMap {
    concepts: Vec<String>,
    index: HashMap<String, usize>,
    lam: f64,
    self_memory: bool,
    weights: HashMap<Edge, f64>,
    w_in: Vec<Vec<(usize, f64)>>,
}

/// Como a inferência terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    FixedPoint,
    LimitCycle,
    MaxIter,
}

/// Resultado de uma inferência.
#[derive(Debug, Clone)]
pub struct FcmResult {
    pub final_state: HashMap<String, f64>,
    pub iterations: usize,
    pub converged: bool,
    pub kind: Termination,
}

impl FcmResult {
    /// Conceito de maior ativação (restrito a `among`).
    pub fn top_concept(&self, among: &[String]) -> (String, f64) {
        let amongset: HashSet<&String> = among.iter().collect();
        self.final_state
            .iter()
            .filter(|(k, _)| amongset.contains(k))
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, v)| (k.clone(), *v))
            .expect("nenhum conceito para desfuzificar")
    }
}

impl FuzzyCognitiveMap {
    pub fn new(
        concepts: Vec<String>,
        weights: HashMap<Edge, f64>,
        lam: f64,
        self_memory: bool,
    ) -> Result<Self, FcmError> {
        let index: HashMap<String, usize> = concepts
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, c)| (c, i))
            .collect();
        if index.len() != concepts.len() {
            return Err(FcmError::DuplicateConcept);
        }
        for ((src, dst), w) in &weights {
            if !index.contains_key(src) || !index.contains_key(dst) {
                return Err(FcmError::UnknownConcept(src.clone(), dst.clone()));
            }
            if !(-1.0 - 1e-9..=1.0 + 1e-9).contains(w) {
                return Err(FcmError::WeightOutOfRange(src.clone(), dst.clone(), *w));
            }
        }
        let mut m = Self {
            index,
            concepts,
            lam,
            self_memory,
            weights,
            w_in: Vec::new(),
        };
        m.rebuild_incoming();
        Ok(m)
    }

    fn rebuild_incoming(&mut self) {
        let n = self.concepts.len();
        self.w_in = vec![Vec::new(); n];
        for ((src, dst), w) in &self.weights {
            if *w != 0.0 {
                let (s, d) = (self.index[src], self.index[dst]);
                self.w_in[d].push((s, *w));
            }
        }
    }

    pub fn get_weights(&self) -> HashMap<Edge, f64> {
        self.weights.clone()
    }

    pub fn set_weights(&mut self, weights: HashMap<Edge, f64>) -> Result<(), FcmError> {
        for ((src, dst), w) in &weights {
            if !self.index.contains_key(src) || !self.index.contains_key(dst) {
                return Err(FcmError::UnknownConcept(src.clone(), dst.clone()));
            }
            if !(-1.0 - 1e-9..=1.0 + 1e-9).contains(w) {
                return Err(FcmError::WeightOutOfRange(src.clone(), dst.clone(), *w));
            }
        }
        self.weights = weights;
        self.rebuild_incoming();
        Ok(())
    }

    fn step(&self, state: &[f64], clamp: &HashMap<usize, f64>) -> Vec<f64> {
        let mut nxt = vec![0.0; self.concepts.len()];
        for c in 0..self.concepts.len() {
            // Conceitos driver (entradas medidas) ficam FIXOS — padrão FCM para decisão.
            if let Some(&v) = clamp.get(&c) {
                nxt[c] = v;
                continue;
            }
            let influence: f64 = self.w_in[c].iter().map(|&(s, w)| state[s] * w).sum();
            let base = if self.self_memory {
                state[c] + influence
            } else {
                influence
            };
            nxt[c] = sigmoid(base, self.lam);
        }
        nxt
    }

    /// Inferência: ponto fixo (delta < epsilon) ou ciclo-limite (estado repetido)
    /// ou max_iter. Estado inicial: conceitos ausentes = 0.0; `clamp` lista os
    /// conceitos de ENTRADA mantidos fixos (padrão FCM de decisão).
    pub fn infer(
        &self,
        initial_state: &HashMap<String, f64>,
        max_iter: usize,
        epsilon: f64,
        clamp: &[String],
    ) -> FcmResult {
        let mut state: Vec<f64> = self
            .concepts
            .iter()
            .map(|c| *initial_state.get(c).unwrap_or(&0.0))
            .collect();
        let clamp_values: HashMap<usize, f64> = clamp
            .iter()
            .filter_map(|c| self.index.get(c).map(|&i| (i, state[i])))
            .collect();

        let key = |s: &[f64]| -> Vec<i64> { s.iter().map(|v| (v * 1e4).round() as i64).collect() };
        let mut seen: HashMap<Vec<i64>, usize> = HashMap::new();
        seen.insert(key(&state), 0);

        for it in 1..=max_iter {
            let nxt = self.step(&state, &clamp_values);
            let delta = (0..state.len())
                .map(|c| (nxt[c] - state[c]).abs())
                .fold(0.0f64, f64::max);
            if delta < epsilon {
                return FcmResult {
                    final_state: self.to_map(nxt),
                    iterations: it,
                    converged: true,
                    kind: Termination::FixedPoint,
                };
            }
            let k = key(&nxt);
            if seen.contains_key(&k) {
                return FcmResult {
                    final_state: self.to_map(nxt),
                    iterations: it,
                    converged: false,
                    kind: Termination::LimitCycle,
                };
            }
            seen.insert(k, it);
            state = nxt;
        }

        FcmResult {
            final_state: self.to_map(state),
            iterations: max_iter,
            converged: false,
            kind: Termination::MaxIter,
        }
    }

    fn to_map(&self, state: Vec<f64>) -> HashMap<String, f64> {
        self.concepts.iter().cloned().zip(state).collect()
    }
}

// ── profile.rs: configuração do FCM de QoS ─────────────────────────────────

pub const INPUT_CONCEPTS: [&str; 8] = [
    "urgency",
    "deadline_pressure",
    "recent_latency",
    "agent_load",
    "error_rate",
    "historical_confidence",
    "estimated_complexity",
    "streaming_need",
];

pub const DECISION_CONCEPTS: [&str; 5] = [
    "QoS_Critical",
    "QoS_Failover",
    "QoS_StreamLike",
    "QoS_LowCost",
    "QoS_Balanced",
];

/// Matriz causal v1 (semente especialista) — idêntica ao `profile.py`:
/// arestas entrada→decisão do QoSSelector (sem os não-medidos recent_ttft/allowed_cost)
/// + arestas ENTRE conceitos (o que distingue FCM de linear).
pub fn build_weight_matrix() -> HashMap<Edge, f64> {
    let mut w = HashMap::new();
    let mut edge = |dst: &str, src: &str, weight: f64| {
        w.insert((src.to_string(), dst.to_string()), weight);
    };

    // QoS_Critical
    edge("QoS_Critical", "urgency", 0.30);
    edge("QoS_Critical", "deadline_pressure", 0.20);
    edge("QoS_Critical", "recent_latency", -0.15);
    edge("QoS_Critical", "historical_confidence", 0.10);
    edge("QoS_Critical", "agent_load", -0.10);
    edge("QoS_Critical", "estimated_complexity", 0.05);
    // QoS_Failover
    edge("QoS_Failover", "error_rate", 0.25);
    edge("QoS_Failover", "recent_latency", 0.20);
    edge("QoS_Failover", "agent_load", 0.15);
    edge("QoS_Failover", "historical_confidence", -0.15);
    edge("QoS_Failover", "deadline_pressure", 0.15);
    edge("QoS_Failover", "urgency", 0.10);
    // QoS_StreamLike
    edge("QoS_StreamLike", "streaming_need", 0.35);
    edge("QoS_StreamLike", "urgency", 0.20);
    edge("QoS_StreamLike", "recent_latency", -0.10);
    edge("QoS_StreamLike", "agent_load", -0.10);
    edge("QoS_StreamLike", "historical_confidence", 0.10);
    // QoS_LowCost
    edge("QoS_LowCost", "urgency", -0.30);
    edge("QoS_LowCost", "estimated_complexity", -0.25);
    edge("QoS_LowCost", "streaming_need", -0.15);
    edge("QoS_LowCost", "deadline_pressure", -0.10);
    // QoS_Balanced
    edge("QoS_Balanced", "error_rate", -0.20);
    edge("QoS_Balanced", "historical_confidence", 0.20);
    edge("QoS_Balanced", "agent_load", -0.15);
    edge("QoS_Balanced", "recent_latency", -0.15);
    edge("QoS_Balanced", "urgency", -0.15);

    // Arestas ENTRE conceitos (causalidade que o score linear não tem)
    edge("recent_latency", "agent_load", 0.30); // carga alta → mais latência
    edge("historical_confidence", "error_rate", -0.40); // erros derrubam confiança
    edge("urgency", "deadline_pressure", 0.30); // pressão de prazo → urgência

    w
}

/// FCM de QoS v1 (semente especialista), lam=1.0, self_memory=true (defaults do Python).
pub fn build_qos_fcm() -> FuzzyCognitiveMap {
    let concepts: Vec<String> = INPUT_CONCEPTS
        .iter()
        .chain(DECISION_CONCEPTS.iter())
        .map(|s| s.to_string())
        .collect();
    FuzzyCognitiveMap::new(concepts, build_weight_matrix(), 1.0, true)
        .expect("configuração v1 válida")
}

/// Roda a inferência com os conceitos de ENTRADA clampados (como `decide_qos`).
pub fn decide_qos(
    fcm: &FuzzyCognitiveMap,
    metrics: &HashMap<String, f64>,
) -> (String, f64, FcmResult) {
    let clamp: Vec<String> = INPUT_CONCEPTS.iter().map(|s| s.to_string()).collect();
    let r = fcm.infer(metrics, 100, 1e-4, &clamp);
    let among: Vec<String> = DECISION_CONCEPTS.iter().map(|s| s.to_string()).collect();
    let (winner, score) = r.top_concept(&among);
    (winner, score, r)
}

// ── DHL (dhl.py) ───────────────────────────────────────────────────────────

fn clamp_w(w: f64) -> f64 {
    w.clamp(-1.0, 1.0)
}

/// Aprendizado Hebbiano Diferencial (Kosko 1988):
/// `Δw = c_t·(ΔC_i·ΔC_j − w)` com `c_t = c0·decay^t` (convergente).
pub struct DifferentialHebbianLearner {
    c0: f64,
    decay: f64,
    t: u64,
}

impl DifferentialHebbianLearner {
    pub fn new(learning_rate: f64, decay: f64) -> Self {
        Self {
            c0: learning_rate,
            decay,
            t: 0,
        }
    }

    pub fn reset(&mut self) {
        self.t = 0;
    }

    /// Um passo de DHL sobre as arestas em `weights` (mut in-place).
    pub fn update_step(
        &mut self,
        weights: &mut HashMap<Edge, f64>,
        prev_state: &HashMap<String, f64>,
        curr_state: &HashMap<String, f64>,
    ) {
        let c_t = self.c0 * self.decay.powi(self.t as i32);
        let keys: Vec<Edge> = weights.keys().cloned().collect();
        for (i, j) in keys {
            let d_i = curr_state.get(&i).copied().unwrap_or(0.0)
                - prev_state.get(&i).copied().unwrap_or(0.0);
            let d_j = curr_state.get(&j).copied().unwrap_or(0.0)
                - prev_state.get(&j).copied().unwrap_or(0.0);
            let w = weights.get(&(i.clone(), j.clone())).copied().unwrap_or(0.0);
            weights.insert((i, j), clamp_w(w + c_t * (d_i * d_j - w)));
        }
        self.t += 1;
    }

    /// DHL sobre pares consecutivos de uma série de estados.
    pub fn learn_from_series(
        &mut self,
        weights: &mut HashMap<Edge, f64>,
        series: &[HashMap<String, f64>],
        reset: bool,
    ) {
        if reset {
            self.reset();
        }
        for pair in series.windows(2) {
            self.update_step(weights, &pair[0], &pair[1]);
        }
    }
}

// ── Fachadas QosDecider ────────────────────────────────────────────────────

fn metrics_to_state(m: &QoSMetrics) -> HashMap<String, f64> {
    [
        ("urgency", m.urgency),
        ("deadline_pressure", m.deadline_pressure),
        ("recent_latency", m.recent_latency),
        ("agent_load", m.agent_load),
        ("error_rate", m.error_rate),
        ("historical_confidence", m.historical_confidence),
        ("estimated_complexity", m.estimated_complexity),
        ("streaming_need", m.streaming_need),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect()
}

fn profile_from_winner(winner: &str) -> QoSProfile {
    match winner {
        "QoS_Critical" => QoSProfile::Critical,
        "QoS_Failover" => QoSProfile::Failover,
        "QoS_StreamLike" => QoSProfile::StreamLike,
        "QoS_LowCost" => QoSProfile::LowCost,
        _ => QoSProfile::Balanced,
    }
}

/// `QosDecider` FCM puro (pesos fixos da semente especialista).
pub struct FcmDecider {
    fcm: FuzzyCognitiveMap,
}

impl Default for FcmDecider {
    fn default() -> Self {
        Self::new()
    }
}

impl FcmDecider {
    pub fn new() -> Self {
        Self {
            fcm: build_qos_fcm(),
        }
    }

    pub fn fcm(&self) -> &FuzzyCognitiveMap {
        &self.fcm
    }
}

impl QosDecider for FcmDecider {
    fn decide(&self, metrics: &QoSMetrics) -> QoSDecision {
        let state = metrics_to_state(metrics);
        let (winner, score, r) = decide_qos(&self.fcm, &state);
        QoSDecision {
            profile: profile_from_winner(&winner),
            confidence: score,
            explanation: format!(
                "fcm: {} (activation={:.3}, it={}, {:?})",
                winner, score, r.iterations, r.kind
            ),
        }
    }

    fn name(&self) -> &str {
        "fcm"
    }
}

/// `QosDecider` FCM+DHL: aprendizado online a cada `decide` (estado anterior
/// guardado com interior mutability; c_t decai por passo — Kosko 1988).
pub struct FcmDhlDecider {
    inner: std::sync::Mutex<FcmDhlInner>,
}

struct FcmDhlInner {
    fcm: FuzzyCognitiveMap,
    learner: DifferentialHebbianLearner,
    prev_state: Option<HashMap<String, f64>>,
}

impl FcmDhlDecider {
    pub fn new(learning_rate: f64) -> Self {
        Self {
            inner: std::sync::Mutex::new(FcmDhlInner {
                fcm: build_qos_fcm(),
                learner: DifferentialHebbianLearner::new(learning_rate, 0.98),
                prev_state: None,
            }),
        }
    }

    /// Peso atual de uma aresta (para testes/observabilidade).
    pub fn weight_of(&self, src: &str, dst: &str) -> Option<f64> {
        let inner = self.inner.lock().unwrap();
        inner
            .fcm
            .get_weights()
            .get(&(src.to_string(), dst.to_string()))
            .copied()
    }
}

impl Default for FcmDhlDecider {
    fn default() -> Self {
        Self::new(0.1)
    }
}

impl QosDecider for FcmDhlDecider {
    fn decide(&self, metrics: &QoSMetrics) -> QoSDecision {
        let input = metrics_to_state(metrics);
        let mut inner = self.inner.lock().unwrap();

        // Infere com os pesos atuais (estado COMPLETO inclui conceitos de decisão)
        let (winner, score, r) = decide_qos(&inner.fcm, &input);

        // DHL: aprende com a transição do estado completo anterior → atual.
        // Assim as arestas métrica→decisão também aprendem (as variações das
        // ativações de decisão entram no produto), e não decaem para zero.
        if let Some(prev) = inner.prev_state.replace(r.final_state.clone()) {
            let mut w = inner.fcm.get_weights();
            inner.learner.update_step(&mut w, &prev, &r.final_state);
            inner.fcm.set_weights(w).expect("pesos DHL válidos");
        }

        QoSDecision {
            profile: profile_from_winner(&winner),
            confidence: score,
            explanation: format!(
                "fcm-dhl: {} (activation={:.3}, it={}, passo t={})",
                winner, score, r.iterations, inner.learner.t
            ),
        }
    }

    fn name(&self) -> &str {
        "fcm-dhl"
    }
}
