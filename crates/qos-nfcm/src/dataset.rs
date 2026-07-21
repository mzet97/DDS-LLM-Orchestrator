//! Construção do dataset de treino (sintético rotulado + cenários canônicos).
//! Rotular dados reais exige contrafactuais (cada perfil no mesmo estado) — ver
//! o loader Python `neuro_fuzzy/dataset.py`. Aqui: PRNG próprio (sem deps).

use crate::nfcm::N_METRICS;

#[derive(Clone)]
pub struct Sample {
    pub x: [f64; N_METRICS],
    pub q_star: Option<usize>,
}

/// Cenários canônicos: (vetor de métricas, perfil esperado).
/// Índices de perfil: 0=Critical 1=Failover 2=StreamLike 3=LowCost 4=Balanced.
pub const CANONICAL: [([f64; N_METRICS], usize); 4] = [
    ([0.10, 0.05, 0.15, 0.10, 0.05, 0.90, 0.20, 0.05], 4), // ocioso  -> Balanced
    ([0.95, 0.90, 0.30, 0.35, 0.10, 0.85, 0.50, 0.10], 0), // urgência-> Critical
    ([0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10], 1), // degradado->Failover
    ([0.50, 0.30, 0.40, 0.45, 0.10, 0.80, 0.40, 0.92], 2), // streaming->StreamLike
];

/// PRNG determinístico (splitmix64) + ruído gaussiano (Box-Muller).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_add(0x9E3779B97F4A7C15))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn gauss(&mut self, sigma: f64) -> f64 {
        let u1 = self.unit().max(1e-12);
        let u2 = self.unit();
        sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// Amostras rotuladas em torno dos cenários canônicos (ruído gaussiano).
pub fn synthetic_dataset(n_per_class: usize, noise: f64, seed: u64) -> Vec<Sample> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(n_per_class * CANONICAL.len());
    for (base, label) in CANONICAL.iter() {
        for _ in 0..n_per_class {
            let mut x = [0.0f64; N_METRICS];
            for i in 0..N_METRICS {
                x[i] = (base[i] + rng.gauss(noise)).clamp(0.0, 1.0);
            }
            out.push(Sample {
                x,
                q_star: Some(*label),
            });
        }
    }
    // embaralha (Fisher-Yates determinístico)
    for i in (1..out.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        out.swap(i, j);
    }
    out
}

/// Split treino/validação/teste. `temporal=true` não embaralha (evita vazamento).
pub fn split(s: &[Sample], f_train: f64, f_val: f64) -> (Vec<Sample>, Vec<Sample>, Vec<Sample>) {
    let n = s.len();
    let a = (f_train * n as f64) as usize;
    let b = ((f_train + f_val) * n as f64) as usize;
    (s[..a].to_vec(), s[a..b].to_vec(), s[b..].to_vec())
}
