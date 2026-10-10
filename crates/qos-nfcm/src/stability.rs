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
    ///
    /// Off-by-one corrigido (REQ/T-820-08): o teste de bloqueio usa o valor
    /// do cooldown ANTES do decremento do ciclo — antes, o decremento no
    /// topo fazia `cooldown_left == 0` bater um ciclo cedo demais e
    /// `cooldown = N` produzia N−1 ticks de bloqueio efetivo. O ciclo agora
    /// é: troca → bloqueia os próximos `cooldown` updates → libera.
    pub fn update(&mut self, winner: usize, confidence: f64, runner_up: f64) -> usize {
        let cfg = self.cfg;
        self.dwell = self.dwell.saturating_add(1);
        // Estado do cooldown DESTE ciclo (antes do decremento — o decremento
        // é por update recebido, o bloqueio vale para o ciclo inteiro).
        let in_cooldown = self.cooldown_left > 0;
        if self.cooldown_left > 0 {
            self.cooldown_left -= 1;
        }
        if confidence < cfg.min_confidence {
            // Confiança baixa: mantém o perfil corrente e zera o processo de
            // troca (candidato/streak) — uma sequência interrompida por ruído
            // não pode retomar a contagem de persistência.
            self.candidate = None;
            self.streak = 0;
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
            && !in_cooldown;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_custom(persist_k: u32, min_dwell: u32, cooldown: u32) -> StabilityConfig {
        StabilityConfig {
            margin_m: 0.10,
            persist_k,
            min_dwell,
            cooldown,
            min_confidence: 0.30,
            fallback: 4,
        }
    }

    /// REQ/T-820-08: cooldown=N bloqueia N updates completos (antes, o
    /// decremento no topo fazia o teste bater cedo e valer N−1).
    #[test]
    fn cooldown_bloqueia_n_updates_completos() {
        let mut c = StabilityController::new(cfg_custom(1, 0, 2));
        assert_eq!(c.update(1, 0.9, 0.0), 1, "primeira decisão inicializa");
        assert_eq!(c.update(2, 0.9, 0.0), 2, "troca inicia cooldown=2");
        assert_eq!(c.update(1, 0.9, 0.0), 2, "bloqueado (cooldown 2→1)");
        assert_eq!(c.update(1, 0.9, 0.0), 2, "bloqueado (cooldown 1→0)");
        assert_eq!(c.update(1, 0.9, 0.0), 1, "cooldown expirado → troca");
    }

    /// REQ/T-820-08: confiança baixa reseta candidato/streak — sequência de
    /// troca interrompida por ruído não retoma a contagem de persistência.
    #[test]
    fn confianca_baixa_reseta_candidato_e_streak() {
        let mut c = StabilityController::new(cfg_custom(2, 0, 0));
        assert_eq!(c.update(1, 0.9, 0.0), 1, "primeira decisão inicializa");
        assert_eq!(c.update(2, 0.9, 0.0), 1, "candidato 2, streak=1 → mantém");
        assert_eq!(c.update(2, 0.2, 0.0), 1, "confiança baixa → mantém");
        assert_eq!(
            c.update(2, 0.9, 0.0),
            1,
            "streak reiniciou (não retomou do ruído) → mantém"
        );
        assert_eq!(c.update(2, 0.9, 0.0), 2, "streak=2 (persist_k) → troca");
    }

    /// Primeira decisão com confiança baixa instala o fallback (Balanced).
    #[test]
    fn primeira_decisao_com_confianca_baixa_instala_fallback() {
        let mut c = StabilityController::new(Default::default());
        assert_eq!(c.update(0, 0.1, 0.0), 4, "fallback=4 (QoS_Balanced)");
        assert_eq!(c.current(), Some(4));
    }
}
