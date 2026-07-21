//! Gerador de workload — porte fiel de
//! `benchmarks/experiments/real_workload_driver.py`:
//!
//! - chegadas por processo de Poisson com λ variável (bursts na janela
//!   `[0, burst_duration_s)` de cada ciclo de `burst_interval_s`);
//! - tokens de prompt ~ LogNormal(ln(mean), 0.5), clamp [32, 2048];
//! - prompt sintético "~4 tokens/word", cap de 50 palavras + marcador.
//!
//! E de `workload_driver.py` (modo simulado): piso de λ em 1.0 quando a
//! config zera (a versão real usa 0.1 — mantemos 0.1, da versão real que
//! roda no cluster).

use crate::regimes::WorkloadConfig;
use crate::rng::Rng;

/// Piso de λ quando a config zera (linha 145-146 do real_workload_driver.py).
const LAMBDA_FLOOR: f64 = 0.1;
/// Clamp de tokens do prompt (linhas 105 e 113 do Python).
const MIN_PROMPT_TOKENS: u32 = 32;
const MAX_PROMPT_TOKENS: u32 = 2048;
/// σ da lognormal de tokens (fixo 0.5 no Python).
const PROMPT_SIGMA: f64 = 0.5;

pub struct WorkloadGenerator {
    cfg: WorkloadConfig,
    rng: Rng,
}

impl WorkloadGenerator {
    pub fn new(cfg: WorkloadConfig, seed: u64) -> Self {
        Self {
            cfg,
            rng: Rng::new(seed),
        }
    }

    pub fn config(&self) -> &WorkloadConfig {
        &self.cfg
    }

    /// λ efetivo no instante `elapsed_s` (com burst) — porte das linhas
    /// 136-146: burst ativo quando `elapsed % interval < duration`.
    pub fn lambda_at(&self, elapsed_s: f64) -> f64 {
        let mut lambda = self.cfg.lambda_rps;
        if self.cfg.burst_lambda_rps > 0.0
            && self.cfg.burst_interval_s > 0.0
            && (elapsed_s % self.cfg.burst_interval_s) < self.cfg.burst_duration_s
        {
            lambda += self.cfg.burst_lambda_rps;
        }
        if lambda <= 0.0 {
            lambda = LAMBDA_FLOOR;
        }
        lambda
    }

    /// Próximo inter-arrival em segundos (exponencial com o λ do instante).
    pub fn next_inter_arrival(&mut self, elapsed_s: f64) -> f64 {
        self.rng.exponential(self.lambda_at(elapsed_s))
    }

    /// Tokens do prompt: LogNormal(ln(mean), 0.5) com clamp [32, 2048].
    pub fn prompt_tokens(&mut self) -> u32 {
        let mu = (self.cfg.prompt_mean_tokens as f64).ln();
        let n = self.rng.lognormal(mu, PROMPT_SIGMA) as i64;
        n.clamp(MIN_PROMPT_TOKENS as i64, MAX_PROMPT_TOKENS as i64) as u32
    }

    /// Prompt sintético (~4 tokens/word, cap de 50 palavras) — porte de
    /// `generate_prompt`: `"word0 word1 ... [prompt com ~N tokens]"`.
    pub fn generate_prompt(&mut self) -> String {
        let n_tokens = self.prompt_tokens() as usize;
        let mut prompt = String::with_capacity(n_tokens.min(50) * 7 + 40);
        for (i, word) in (0..n_tokens / 4)
            .map(|i| format!("word{i}"))
            .take(50)
            .enumerate()
        {
            if i > 0 {
                prompt.push(' ');
            }
            prompt.push_str(&word);
        }
        prompt.push_str(&format!(" [prompt com ~{n_tokens} tokens]"));
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regimes::{LEVE, PESADA};

    #[test]
    fn lambda_sem_burst_e_constante() {
        let gen = WorkloadGenerator::new(LEVE, 42);
        for t in [0.0, 0.4, 9.9, 10.0, 100.3] {
            assert_eq!(gen.lambda_at(t), 5.0);
        }
    }

    #[test]
    fn lambda_com_burst_na_janela() {
        let gen = WorkloadGenerator::new(PESADA, 42);
        // Janela de burst: [0, 0.5) de cada ciclo de 10 s.
        assert_eq!(gen.lambda_at(0.0), 80.0);
        assert_eq!(gen.lambda_at(0.49), 80.0);
        assert_eq!(gen.lambda_at(0.5), 30.0);
        assert_eq!(gen.lambda_at(9.99), 30.0);
        assert_eq!(gen.lambda_at(10.0), 80.0);
        assert_eq!(gen.lambda_at(20.25), 80.0);
    }

    #[test]
    fn inter_arrival_medio_bate_um_sobre_lambda() {
        let mut gen = WorkloadGenerator::new(LEVE, 42);
        let n = 20_000;
        // elapsed > 0.5 para ficar fora da janela de burst (leve não tem burst,
        // mas a forma geral importa).
        let mean: f64 = (0..n).map(|_| gen.next_inter_arrival(1.0)).sum::<f64>() / n as f64;
        let esperado = 1.0 / 5.0;
        assert!(
            (mean - esperado).abs() / esperado < 0.03,
            "média {mean} vs esperada {esperado}"
        );
    }

    #[test]
    fn prompt_tokens_dentro_do_clamp() {
        let mut gen = WorkloadGenerator::new(LEVE, 7);
        for _ in 0..10_000 {
            let t = gen.prompt_tokens();
            assert!((MIN_PROMPT_TOKENS..=MAX_PROMPT_TOKENS).contains(&t));
        }
    }

    #[test]
    fn prompt_respeita_cap_de_palavras() {
        let mut gen = WorkloadGenerator::new(LEVE, 3);
        for _ in 0..100 {
            let p = gen.generate_prompt();
            let words = p.split_whitespace().count();
            // ≤ 50 palavras geradas + o marcador final (5 "palavras").
            assert!(words <= 55, "words={words}");
            assert!(p.contains("[prompt com ~"));
        }
    }

    #[test]
    fn deterministico_por_seed() {
        let mut a = WorkloadGenerator::new(PESADA, 42);
        let mut b = WorkloadGenerator::new(PESADA, 42);
        for _ in 0..1000 {
            assert_eq!(a.next_inter_arrival(1.0), b.next_inter_arrival(1.0));
            assert_eq!(a.prompt_tokens(), b.prompt_tokens());
        }
    }
}
