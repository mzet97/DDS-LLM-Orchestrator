//! Fuzificação por gaussianas com parâmetros treináveis.
//!
//! μ_ik(x) = exp(−(x − c_ik)² / (2 σ_ik²)),  σ_ik = softplus(r_ik) + ε (> 0).
//! Porte fiel de `neuro_fuzzy/membership.py`.

pub const EPS: f64 = 1e-3;

#[inline]
pub fn softplus(r: f64) -> f64 {
    if r > 30.0 {
        r
    } else {
        (1.0 + r.exp()).ln()
    }
}

#[inline]
pub fn sigma_from_r(r: f64) -> f64 {
    softplus(r) + EPS
}

/// Inverso: r tal que softplus(r)+ε = σ (inicialização por especialista).
pub fn r_from_sigma(sigma: f64) -> f64 {
    let target = (sigma - EPS).max(1e-6);
    if target < 30.0 {
        (target.exp_m1()).ln()
    } else {
        target
    }
}

#[inline]
pub fn gaussian(x: f64, c: f64, sigma: f64) -> f64 {
    (-((x - c).powi(2)) / (2.0 * sigma * sigma)).exp()
}

/// Termo linguístico gaussiano treinável (centro `c`, parâmetro de largura `r`).
#[derive(Clone, Copy, Debug)]
pub struct GaussTerm {
    pub c: f64,
    pub r: f64,
}

impl GaussTerm {
    #[inline]
    pub fn sigma(&self) -> f64 {
        sigma_from_r(self.r)
    }
    #[inline]
    pub fn mu(&self, x: f64) -> f64 {
        gaussian(x, self.c, self.sigma())
    }
}

/// Termos padrão baixo/medio/alto (centros 0/0.5/1, σ≈0.25).
pub fn default_terms() -> [GaussTerm; 3] {
    let r = r_from_sigma(0.25);
    [
        GaussTerm { c: 0.0, r }, // baixo
        GaussTerm { c: 0.5, r }, // medio
        GaussTerm { c: 1.0, r }, // alto
    ]
}

/// Índice do termo "alto" nos arrays de 3 termos.
pub const ALTO: usize = 2;

/// Fuzifica um valor em [0,1] para os 3 graus de pertinência.
#[inline]
pub fn fuzzify(value: f64, terms: &[GaussTerm; 3]) -> [f64; 3] {
    let v = value.clamp(0.0, 1.0);
    [terms[0].mu(v), terms[1].mu(v), terms[2].mu(v)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softplus_reparam_positiva() {
        assert!((sigma_from_r(r_from_sigma(0.25)) - 0.25).abs() < 1e-9);
        assert!(sigma_from_r(-50.0) > 0.0);
        assert!((gaussian(0.5, 0.5, 0.25) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn reproduz_pertinencias_do_artigo() {
        let t = default_terms();
        let m = fuzzify(0.90, &t); // error_rate
        assert!((m[ALTO] - 0.9231).abs() < 5e-3);
    }
}
