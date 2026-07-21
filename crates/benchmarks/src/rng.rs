//! RNG determinístico (xoshiro256**) + amostradores — zero deps, sem `rand`.
//!
//! Reprodutibilidade é requisito de benchmark: mesma seed ⇒ mesma sequência.
//! Os PARÂMETROS de distribuição espelham o NumPy do Python
//! (`np.random.default_rng(seed)`): mesma λ, mesmo μ/σ da lognormal, mesmos
//! clamps. As SEQUÊNCIAS diferem por algoritmo (PCG64 ≠ xoshiro256**) — a
//! paridade com o Python é estatística, não bit-a-bit (ver tests).

/// Estado xoshiro256** (Blackman & Vigna, 2018). Período 2^256 − 1.
pub struct Rng([u64; 4]);

impl Rng {
    /// Expande a seed via SplitMix64 (mesma técnica do `Random` de Java/NumPy
    /// para derivar estado maior que a seed).
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let mut next = move || {
            sm = sm.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = sm;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        Self([next(), next(), next(), next()])
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let [s0, s1, s2, s3] = &mut self.0;
        let result = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = *s1 << 17;
        *s2 ^= *s0;
        *s3 ^= *s1;
        *s1 ^= *s2;
        *s0 ^= *s3;
        *s2 ^= t;
        *s3 = s3.rotate_left(45);
        result
    }

    /// Uniforme em [0, 1) — 53 bits de precisão (como `rng.random()` do NumPy).
    #[inline]
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniforme em `[low, high)` (porte de `rng.uniform(low, high)`).
    #[inline]
    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.f64()
    }

    /// Exponencial com média `1/lambda` (porte de `rng.exponential(1/λ)`).
    /// Método da transformada inversa: `-ln(1-u)/λ`.
    #[inline]
    pub fn exponential(&mut self, lambda: f64) -> f64 {
        debug_assert!(lambda > 0.0);
        -(1.0 - self.f64()).ln() / lambda
    }

    /// Normal padrão via Box-Muller (para a lognormal).
    #[inline]
    pub fn standard_normal(&mut self) -> f64 {
        let u1 = 1.0 - self.f64(); // (0, 1] — evita ln(0)
        let u2 = self.f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// LogNormal cujo log tem média `mean_log` e desvio `sigma`
    /// (porte de `rng.lognormal(mean=log(512), sigma=0.5)`).
    #[inline]
    pub fn lognormal(&mut self, mean_log: f64, sigma: f64) -> f64 {
        (mean_log + sigma * self.standard_normal()).exp()
    }

    /// Escolha ponderada entre 3 valores (porte de
    /// `rng.choice([0.0, 0.5, 1.0], p=[0.5, 0.3, 0.2])`).
    pub fn choice3_weighted(&mut self, values: [f64; 3], p: [f64; 3]) -> f64 {
        debug_assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        let u = self.f64();
        if u < p[0] {
            values[0]
        } else if u < p[0] + p[1] {
            values[1]
        } else {
            values[2]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesma_seed_mesma_sequencia() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn seeds_diferentes_divergem() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn exponencial_tem_media_um_sobre_lambda() {
        let mut rng = Rng::new(42);
        let lambda = 15.0;
        let n = 50_000;
        let mean: f64 = (0..n).map(|_| rng.exponential(lambda)).sum::<f64>() / n as f64;
        let esperado = 1.0 / lambda;
        assert!(
            (mean - esperado).abs() / esperado < 0.02,
            "média {mean} vs esperada {esperado}"
        );
    }

    #[test]
    fn lognormal_tem_mediana_exp_mu() {
        let mut rng = Rng::new(42);
        let mu = 512.0_f64.ln();
        let mut xs: Vec<f64> = (0..50_000).map(|_| rng.lognormal(mu, 0.5)).collect();
        xs.sort_by(f64::total_cmp);
        let mediana = xs[xs.len() / 2];
        assert!(
            (mediana - 512.0).abs() / 512.0 < 0.02,
            "mediana {mediana} vs 512"
        );
    }

    #[test]
    fn choice_respeita_probabilidades() {
        let mut rng = Rng::new(42);
        let n = 30_000;
        let mut counts = [0u32; 3];
        for _ in 0..n {
            let v = rng.choice3_weighted([0.0, 0.5, 1.0], [0.5, 0.3, 0.2]);
            counts[(v * 2.0) as usize] += 1;
        }
        let p0 = counts[0] as f64 / n as f64;
        assert!((p0 - 0.5).abs() < 0.02, "p0={p0}");
    }
}
