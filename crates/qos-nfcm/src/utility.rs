//! Função de utilidade multiobjetivo U(q|x) — rótulo-alvo do treino supervisionado.
//! Coeficientes são parâmetros de projeto a calibrar (defaults neutros).

#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub l_p95: f64,
    pub deadline_viol: f64,
    pub error_rate: f64,
    pub ttft: f64,
    pub itl: f64,
    pub cost: f64,
    pub throughput: f64,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            l_p95: 0.0,
            deadline_viol: 0.0,
            error_rate: 0.0,
            ttft: 0.0,
            itl: 0.0,
            cost: 0.0,
            throughput: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct UtilityWeights {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub delta: f64,
    pub eta: f64,
    pub zeta: f64,
    pub theta: f64,
}

impl Default for UtilityWeights {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            beta: 1.0,
            gamma: 1.0,
            delta: 0.5,
            eta: 0.5,
            zeta: 0.3,
            theta: 0.5,
        }
    }
}

pub fn utility(o: &Outcome, w: &UtilityWeights) -> f64 {
    -w.alpha * o.l_p95
        - w.beta * o.deadline_viol
        - w.gamma * o.error_rate
        - w.delta * o.ttft
        - w.eta * o.itl
        - w.zeta * o.cost
        + w.theta * o.throughput
}

/// q* = argmax_q U(q|x) dado o resultado medido de cada perfil no mesmo estado.
pub fn best_profile(outcomes: &[(usize, Outcome)], w: &UtilityWeights) -> Option<usize> {
    outcomes
        .iter()
        .map(|(p, o)| (*p, utility(o, w)))
        .fold(None, |acc: Option<(usize, f64)>, (p, u)| match acc {
            Some((_, bu)) if bu >= u => acc,
            _ => Some((p, u)),
        })
        .map(|(p, _)| p)
}
