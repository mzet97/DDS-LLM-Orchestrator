//! Salvaguardas de estabilidade da atuação online (histerese/persistência/cooldown).
//! Determinístico; parâmetros a calibrar. Porte de `neuro_fuzzy/stability.py`.

#[derive(Clone, Copy)]
pub struct StabilityConfig {
    pub margin_m: f64,
    pub persist_k: u32,
    pub min_dwell: u32,
    pub cooldown: u32,
    pub min_confidence: f64,
    pub fallback: usize, // índice do perfil de fallback (QoS_Balanced = 4)
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            margin_m: 0.10,
            persist_k: 2,
            min_dwell: 3,
            cooldown: 2,
            min_confidence: 0.30,
            fallback: 4,
        }
    }
}

pub struct StabilityController {
    pub cfg: StabilityConfig,
    current: Option<usize>,
    dwell: u32,
    cooldown_left: u32,
    candidate: Option<usize>,
    streak: u32,
}

impl StabilityController {
    pub fn new(cfg: StabilityConfig) -> Self {
        Self {
            cfg,
            current: None,
            dwell: 0,
            cooldown_left: 0,
            candidate: None,
            streak: 0,
        }
    }

    /// Recebe a decisão bruta e devolve o perfil EFETIVO a aplicar.
    pub fn update(&mut self, winner: usize, confidence: f64, runner_up: f64) -> usize {
        let cfg = self.cfg;
        self.dwell = self.dwell.saturating_add(1);
        if self.cooldown_left > 0 {
            self.cooldown_left -= 1;
        }
        if confidence < cfg.min_confidence {
            let cur = *self.current.get_or_insert(cfg.fallback);
            return cur;
        }
        let current = match self.current {
            None => {
                self.current = Some(winner);
                self.dwell = 0;
                return winner;
            }
            Some(c) => c,
        };
        if winner == current {
            self.candidate = None;
            self.streak = 0;
            return current;
        }
        let margin_ok = (confidence - runner_up) > cfg.margin_m;
        self.streak = if self.candidate == Some(winner) {
            self.streak + 1
        } else {
            1
        };
        self.candidate = Some(winner);
        let can_switch = margin_ok
            && self.streak >= cfg.persist_k
            && self.dwell >= cfg.min_dwell
            && self.cooldown_left == 0;
        if can_switch {
            self.current = Some(winner);
            self.dwell = 0;
            self.cooldown_left = cfg.cooldown;
            self.candidate = None;
            self.streak = 0;
            winner
        } else {
            current
        }
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }
}
